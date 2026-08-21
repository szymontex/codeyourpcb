//! Claims the design makes about itself.
//!
//! `assert board.width <= 100mm` is a rule the designer wrote, not one the
//! fabricator imposed, and it belongs with the other things the checker
//! enforces. Until now these statements parsed and nothing read them.
//!
//! An assertion the checker cannot evaluate is reported rather than skipped.
//! A statement that quietly does nothing is worse than one that fails: the
//! board looks checked.

use cypcb_core::physical_units::PhysicalUnit;
use cypcb_parser::ast::{AssertExpression, AssertOperand, ComparisonOp};
use cypcb_world::components::{RefDes, TypedValue};
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for evaluating the design's own `assert` statements.
pub struct AssertionRule;

impl DrcRule for AssertionRule {
    fn name(&self) -> &'static str {
        "assertion"
    }

    fn check(&self, world: &mut BoardWorld, _rules: &DesignRules) -> Vec<DrcViolation> {
        let assertions = world.assertions().to_vec();
        if assertions.is_empty() {
            return Vec::new();
        }

        let board = world.board_info();
        let entity = world.board_entity();

        // What each net's own block asks for, by name.
        let nets: std::collections::HashMap<String, cypcb_world::registry::NetConstraints> = world
            .nets()
            .map(|(id, name)| (name.to_string(), id))
            .collect::<Vec<_>>()
            .into_iter()
            .filter_map(|(name, id)| Some((name, world.net_constraints(id)?)))
            .collect();

        // Values the design wrote as quantities, by reference designator.
        let values: std::collections::HashMap<String, TypedValue> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(&RefDes, &TypedValue)>();
            query
                .iter(ecs)
                .map(|(refdes, value)| (refdes.as_str().to_string(), *value))
                .collect()
        };
        let at = cypcb_core::Point::new(cypcb_core::Nm(0), cypcb_core::Nm(0));

        let mut violations = Vec::new();
        for assertion in &assertions {
            let entity = match entity {
                Some(entity) => entity,
                None => continue,
            };

            match evaluate(&assertion.expression, board, &values, &nets) {
                Outcome::Held => {}
                Outcome::Failed(message) | Outcome::Unevaluable(message) => {
                    let mut violation = DrcViolation::assertion(entity, at);
                    violation.message = message;
                    violations.push(violation);
                }
            }
        }

        violations
    }
}

/// What happened when an assertion was checked.
enum Outcome {
    /// The board satisfies it.
    Held,
    /// The board does not.
    Failed(String),
    /// The checker could not tell, which is itself worth reporting.
    Unevaluable(String),
}

/// A number with the kind of thing it measures, so that a length is never
/// compared against a resistance.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Quantity {
    Length,
    Resistance,
    Capacitance,
    Inductance,
    Voltage,
    Current,
    Frequency,
    Power,
    Count,
}

impl Quantity {
    fn name(&self) -> &'static str {
        match self {
            Quantity::Length => "a length",
            Quantity::Resistance => "a resistance",
            Quantity::Capacitance => "a capacitance",
            Quantity::Inductance => "an inductance",
            Quantity::Voltage => "a voltage",
            Quantity::Current => "a current",
            Quantity::Frequency => "a frequency",
            Quantity::Power => "a power",
            Quantity::Count => "a plain number",
        }
    }
}

/// A resolved operand: its value in base units, and what it measures.
#[derive(Clone, Copy, Debug)]
struct Value {
    base: f64,
    quantity: Quantity,
}

fn evaluate(
    expression: &AssertExpression,
    board: Option<(cypcb_world::components::BoardSize, cypcb_world::LayerStack)>,
    values: &std::collections::HashMap<String, TypedValue>,
    nets: &std::collections::HashMap<String, cypcb_world::registry::NetConstraints>,
) -> Outcome {
    match expression {
        AssertExpression::Comparison {
            left, op, right, ..
        } => {
            let (left_value, right_value) = match (
                resolve(left, board, values, nets),
                resolve(right, board, values, nets),
            ) {
                (Ok(l), Ok(r)) => (l, r),
                (Err(why), _) | (_, Err(why)) => {
                    return Outcome::Unevaluable(format!("assertion not checked: {why}"))
                }
            };

            if left_value.quantity != right_value.quantity {
                return Outcome::Unevaluable(format!(
                    "assertion not checked: {} cannot be compared with {}",
                    left_value.quantity.name(),
                    right_value.quantity.name()
                ));
            }

            if holds(left_value.base, *op, right_value.base) {
                Outcome::Held
            } else {
                Outcome::Failed(format!(
                    "assertion failed: {} {} {}, but the board has {}",
                    describe(left),
                    symbol(*op),
                    describe(right),
                    format_base(left_value)
                ))
            }
        }
        // `within` used to answer "not checked: the board model does not carry
        // tolerances yet", and that was reading a harder question than the one
        // asked. A part's own manufacturing tolerance is indeed not in the
        // model. This assertion does not need it: `R1.value within 10kohm +/-
        // 5%` asks whether the value the design **states** falls in a band,
        // and the value and the band are both here.
        AssertExpression::Within { left, target, .. } => {
            let left_value = match resolve(left, board, values, nets) {
                Ok(value) => value,
                Err(why) => return Outcome::Unevaluable(format!("assertion not checked: {why}")),
            };
            let nominal = Value {
                base: target.unit.to_base_f64(target.value),
                quantity: quantity_of(target.unit),
            };
            if left_value.quantity != nominal.quantity {
                return Outcome::Unevaluable(format!(
                    "assertion not checked: {} cannot be compared with {}",
                    left_value.quantity.name(),
                    nominal.quantity.name()
                ));
            }

            let Some(tolerance) = &target.tolerance else {
                return Outcome::Unevaluable(
                    "assertion not checked: `within` needs a band, as in \
                     `within 10kohm +/- 5%` or `within 100nF to 220nF`"
                        .to_string(),
                );
            };

            let (low, high) = match &tolerance.kind {
                cypcb_parser::ast::ToleranceKind::Percentage { value } => {
                    let spread = nominal.base * value / 100.0;
                    (nominal.base - spread, nominal.base + spread)
                }
                cypcb_parser::ast::ToleranceKind::Absolute(spread) => {
                    if quantity_of(spread.unit) != nominal.quantity {
                        return Outcome::Unevaluable(format!(
                            "assertion not checked: a band of {} does not fit {}",
                            quantity_of(spread.unit).name(),
                            nominal.quantity.name()
                        ));
                    }
                    let spread = spread.unit.to_base_f64(spread.value);
                    (nominal.base - spread, nominal.base + spread)
                }
                // `within 100nF to 220nF` - the nominal is the low end and the
                // stated value is the high one, not a spread either side.
                cypcb_parser::ast::ToleranceKind::Range(upper) => {
                    if quantity_of(upper.unit) != nominal.quantity {
                        return Outcome::Unevaluable(format!(
                            "assertion not checked: a range ending in {} does not fit {}",
                            quantity_of(upper.unit).name(),
                            nominal.quantity.name()
                        ));
                    }
                    (nominal.base, upper.unit.to_base_f64(upper.value))
                }
            };

            if left_value.base >= low && left_value.base <= high {
                Outcome::Held
            } else {
                Outcome::Failed(format!(
                    "assertion failed: {} is {}, which is outside {} to {}",
                    describe(left),
                    format_base(left_value),
                    format_base(Value {
                        base: low,
                        quantity: nominal.quantity
                    }),
                    format_base(Value {
                        base: high,
                        quantity: nominal.quantity
                    })
                ))
            }
        }
    }
}

/// Turn an operand into a number the checker can compare.
fn resolve(
    operand: &AssertOperand,
    board: Option<(cypcb_world::components::BoardSize, cypcb_world::LayerStack)>,
    values: &std::collections::HashMap<String, TypedValue>,
    nets: &std::collections::HashMap<String, cypcb_world::registry::NetConstraints>,
) -> Result<Value, String> {
    match operand {
        AssertOperand::Number { value, .. } => Ok(Value {
            base: *value,
            quantity: Quantity::Count,
        }),
        AssertOperand::Dimension(dimension) => Ok(Value {
            base: dimension.to_nm().raw() as f64,
            quantity: Quantity::Length,
        }),
        AssertOperand::Physical(physical) => Ok(Value {
            base: physical.unit.to_base_f64(physical.value),
            quantity: quantity_of(physical.unit),
        }),
        AssertOperand::QualifiedName { parts, .. } => {
            let path = parts.join(".");

            // `R1.value`, when the design wrote it as a quantity.
            if let [refdes, "value"] = parts
                .as_slice()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()[..]
            {
                return match values.get(refdes) {
                    Some(typed) => Ok(Value {
                        base: typed.base(),
                        quantity: quantity_of(typed.unit),
                    }),
                    None => Err(format!(
                        "'{path}' is not a quantity: write `value 10kohm` rather than `value \"10k\"`, \
                         or the checker has only a label to go on"
                    )),
                };
            }

            // `VCC.current`, `VCC.width`, `VCC.clearance` - what a net's own
            // block asks for. The field decides which kind of name this is:
            // a component has a value, a net has the three below. `board` is
            // neither: `board.width` is a length the board has rather than one
            // a net asks for.
            // `board` is not a net, and `board.width` is a length the board
            // has rather than one a net asks for.
            if let [net_name, field @ ("current" | "width" | "clearance")] = parts
                .as_slice()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()[..]
            {
                if net_name == "board" {
                    // Falls through to the board paths below.
                } else {
                    let Some(constraints) = nets.get(net_name) else {
                        return Err(format!(
                            "'{path}' is not a net that states anything: give it a block like \
                         `net {net_name} [{field} ...]`"
                        ));
                    };
                    return match field {
                        "current" => constraints
                            .current_ma
                            .map(|ma| Value {
                                base: ma / 1000.0,
                                quantity: Quantity::Current,
                            })
                            .ok_or_else(|| format!("net '{net_name}' does not state a current")),
                        "width" => constraints
                            .width
                            .map(|width| Value {
                                base: width.raw() as f64,
                                quantity: Quantity::Length,
                            })
                            .ok_or_else(|| format!("net '{net_name}' does not state a width")),
                        _ => constraints
                            .clearance
                            .map(|clearance| Value {
                                base: clearance.raw() as f64,
                                quantity: Quantity::Length,
                            })
                            .ok_or_else(|| format!("net '{net_name}' does not state a clearance")),
                    };
                }
            }

            let Some((size, layers)) = board else {
                return Err(format!("'{path}' needs a board, and none is defined"));
            };
            match path.as_str() {
                "board.width" => Ok(Value {
                    base: size.width.raw() as f64,
                    quantity: Quantity::Length,
                }),
                "board.height" => Ok(Value {
                    base: size.height.raw() as f64,
                    quantity: Quantity::Length,
                }),
                "board.layers" => Ok(Value {
                    base: layers.count as f64,
                    quantity: Quantity::Count,
                }),
                _ => Err(format!(
                    "'{path}' is not something the checker can read yet; it knows board.width, \
                     board.height, board.layers, <part>.value and <net>.current/width/clearance"
                )),
            }
        }
    }
}

fn quantity_of(unit: PhysicalUnit) -> Quantity {
    use PhysicalUnit::*;
    match unit {
        Ohm | KiloOhm | MegaOhm => Quantity::Resistance,
        PicoFarad | NanoFarad | MicroFarad | MilliFarad => Quantity::Capacitance,
        NanoHenry | MicroHenry | MilliHenry | Henry => Quantity::Inductance,
        MilliVolt | Volt | KiloVolt => Quantity::Voltage,
        MicroAmp | MilliAmp | Amp => Quantity::Current,
        Hertz | KiloHertz | MegaHertz | GigaHertz => Quantity::Frequency,
        MilliWatt | Watt => Quantity::Power,
    }
}

fn holds(left: f64, op: ComparisonOp, right: f64) -> bool {
    match op {
        ComparisonOp::Eq => (left - right).abs() < f64::EPSILON.max(right.abs() * 1e-9),
        ComparisonOp::Ne => (left - right).abs() >= f64::EPSILON.max(right.abs() * 1e-9),
        ComparisonOp::Ge => left >= right,
        ComparisonOp::Le => left <= right,
        ComparisonOp::Gt => left > right,
        ComparisonOp::Lt => left < right,
    }
}

fn symbol(op: ComparisonOp) -> &'static str {
    match op {
        ComparisonOp::Eq => "==",
        ComparisonOp::Ne => "!=",
        ComparisonOp::Ge => ">=",
        ComparisonOp::Le => "<=",
        ComparisonOp::Gt => ">",
        ComparisonOp::Lt => "<",
    }
}

fn describe(operand: &AssertOperand) -> String {
    match operand {
        AssertOperand::QualifiedName { parts, .. } => parts.join("."),
        AssertOperand::Physical(physical) => {
            format!("{}{}", physical.value, physical.unit.suffix())
        }
        AssertOperand::Dimension(dimension) => {
            format!("{:.3}mm", dimension.to_nm().raw() as f64 / 1_000_000.0)
        }
        AssertOperand::Number { value, .. } => format!("{value}"),
    }
}

fn format_base(value: Value) -> String {
    match value.quantity {
        Quantity::Length => format!("{:.3}mm", value.base / 1_000_000.0),
        Quantity::Count => format!("{}", value.base),
        // Base SI, so the number is comparable with whatever was claimed even
        // when the claim used a prefix.
        Quantity::Resistance => format!("{}ohm", value.base),
        Quantity::Capacitance => format!("{}F", value.base),
        Quantity::Inductance => format!("{}H", value.base),
        Quantity::Voltage => format!("{}V", value.base),
        Quantity::Current => format!("{}A", value.base),
        Quantity::Frequency => format!("{}Hz", value.base),
        Quantity::Power => format!("{}W", value.base),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_core::Nm;
    use cypcb_parser::ast::{AssertDef, Dimension as AstDimension, PhysicalValue, Span};

    fn board_of(width_mm: f64, height_mm: f64, layers: u8) -> BoardWorld {
        let mut world = BoardWorld::new();
        world.set_board(
            "t".to_string(),
            (Nm::from_mm(width_mm), Nm::from_mm(height_mm)),
            layers,
        );
        world
    }

    /// Build `assert <left> <op> <right>` without a parser: this crate does
    /// not compile one in, and the rule under test only reads the AST.
    fn claim(left: AssertOperand, op: ComparisonOp, right: AssertOperand) -> AssertDef {
        let span = Span::new(0, 0);
        AssertDef {
            expression: AssertExpression::Comparison {
                left,
                op,
                right,
                span,
            },
            span,
        }
    }

    fn name(path: &str) -> AssertOperand {
        AssertOperand::QualifiedName {
            parts: path.split('.').map(str::to_string).collect(),
            span: Span::new(0, 0),
        }
    }

    fn mm(value: f64) -> AssertOperand {
        AssertOperand::Dimension(AstDimension::new(
            value,
            cypcb_core::Unit::Mm,
            Span::new(0, 0),
        ))
    }

    fn count(value: f64) -> AssertOperand {
        AssertOperand::Number {
            value,
            span: Span::new(0, 0),
        }
    }

    fn kilohm(value: f64) -> AssertOperand {
        AssertOperand::Physical(PhysicalValue {
            value,
            unit: PhysicalUnit::KiloOhm,
            tolerance: None,
            span: Span::new(0, 0),
        })
    }

    fn check(claims: Vec<AssertDef>, width_mm: f64, layers: u8) -> Vec<DrcViolation> {
        let mut world = board_of(width_mm, 40.0, layers);
        world.set_assertions(claims);
        AssertionRule.check(&mut world, &DesignRules::jlcpcb_2layer())
    }

    #[test]
    fn a_claim_the_board_meets_is_silent() {
        let claims = vec![claim(name("board.width"), ComparisonOp::Le, mm(100.0))];
        assert!(check(claims, 80.0, 2).is_empty());
    }

    #[test]
    fn a_claim_the_board_breaks_is_reported_with_the_number() {
        let claims = vec![claim(name("board.width"), ComparisonOp::Le, mm(50.0))];
        let violations = check(claims, 80.0, 2);

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].kind, crate::ViolationKind::Assertion);
        assert!(
            violations[0].message.contains("board.width <= 50.000mm")
                && violations[0].message.contains("80.000mm"),
            "the message has to say what was claimed and what is true: {}",
            violations[0].message
        );
    }

    #[test]
    fn layer_count_is_a_plain_number() {
        let ok = vec![claim(name("board.layers"), ComparisonOp::Ge, count(2.0))];
        assert!(check(ok, 80.0, 2).is_empty());

        let bad = vec![claim(name("board.layers"), ComparisonOp::Ge, count(4.0))];
        assert_eq!(check(bad, 80.0, 2).len(), 1);
    }

    #[test]
    fn a_length_is_not_compared_against_a_resistance() {
        let claims = vec![claim(name("board.width"), ComparisonOp::Le, kilohm(10.0))];
        let violations = check(claims, 80.0, 2);

        assert_eq!(violations.len(), 1);
        assert!(
            violations[0].message.contains("cannot be compared"),
            "got {}",
            violations[0].message
        );
    }

    #[test]
    fn a_value_written_as_a_quantity_can_be_checked() {
        use cypcb_world::components::{
            FootprintRef, NetConnections, Position, RefDes, Rotation, TypedValue, Value,
        };

        let mut world = board_of(80.0, 40.0, 2);
        let entity = world.spawn_component(
            RefDes::new("R1"),
            Value::new("10kohm"),
            Position::from_mm(5.0, 5.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );
        world.ecs_mut().entity_mut(entity).insert(TypedValue {
            value: 10.0,
            unit: PhysicalUnit::KiloOhm,
        });

        let holds = vec![claim(name("R1.value"), ComparisonOp::Eq, kilohm(10.0))];
        world.set_assertions(holds);
        assert!(AssertionRule
            .check(&mut world, &DesignRules::jlcpcb_2layer())
            .is_empty());

        let breaks = vec![claim(name("R1.value"), ComparisonOp::Ge, kilohm(47.0))];
        world.set_assertions(breaks);
        let violations = AssertionRule.check(&mut world, &DesignRules::jlcpcb_2layer());
        assert_eq!(violations.len(), 1);
        assert!(
            violations[0].message.contains("assertion failed"),
            "got {}",
            violations[0].message
        );
    }

    #[test]
    fn a_net_can_be_asserted_about() {
        use cypcb_world::registry::NetConstraints;

        let mut world = board_of(80.0, 40.0, 2);
        let vbus = world.intern_net("VBUS");
        world.intern_net("SIG");
        world.set_net_constraints(
            vbus,
            NetConstraints {
                current_ma: Some(2000.0),
                width: Some(Nm::from_mm(0.8)),
                ..Default::default()
            },
        );

        let amp = |value: f64| {
            AssertOperand::Physical(PhysicalValue {
                value,
                unit: PhysicalUnit::Amp,
                tolerance: None,
                span: Span::new(0, 0),
            })
        };

        world.set_assertions(vec![claim(
            name("VBUS.current"),
            ComparisonOp::Ge,
            amp(1.0),
        )]);
        assert!(AssertionRule
            .check(&mut world, &DesignRules::jlcpcb_2layer())
            .is_empty());

        world.set_assertions(vec![claim(name("VBUS.width"), ComparisonOp::Ge, mm(1.0))]);
        let violations = AssertionRule.check(&mut world, &DesignRules::jlcpcb_2layer());
        assert_eq!(violations.len(), 1);
        assert!(
            violations[0].message.contains("0.800mm"),
            "got {}",
            violations[0].message
        );

        // A net that states nothing is not silently treated as satisfying
        // everything.
        world.set_assertions(vec![claim(name("SIG.width"), ComparisonOp::Ge, mm(0.2))]);
        let violations = AssertionRule.check(&mut world, &DesignRules::jlcpcb_2layer());
        assert_eq!(violations.len(), 1);
        assert!(
            violations[0].message.contains("not checked"),
            "got {}",
            violations[0].message
        );
    }

    #[test]
    fn something_the_checker_cannot_read_is_said_out_loud() {
        // The alternative is a statement that silently does nothing, which
        // leaves the board looking checked.
        let claims = vec![claim(name("R1.value"), ComparisonOp::Eq, kilohm(10.0))];
        let violations = check(claims, 80.0, 2);

        assert_eq!(violations.len(), 1);
        assert!(
            violations[0].message.contains("not checked")
                && violations[0].message.contains("R1.value"),
            "got {}",
            violations[0].message
        );
    }
}
