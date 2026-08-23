//! IPC-2141 impedance for a microstrip and a symmetric stripline.
//!
//! # Where these come from, and how far to trust them
//!
//! Two closed forms, both IPC-2141's:
//!
//! ```text
//! microstrip   Z0 = 87 / sqrt(Er + 1.41) * ln( 5.98 H / (0.8 W + T) )
//! stripline    Z0 = 60 / sqrt(Er)        * ln( 4 B / (0.67 pi (0.8 W + T)) )
//! ```
//!
//! `Er` is the dielectric constant, `W` the trace width, `T` the copper
//! thickness, `H` the height of the trace above its reference plane, and `B`
//! the separation between the two planes a stripline runs between.
//!
//! **This project has not checked either against a third party.** IPC-2141 is
//! not a public document; the equations above are the form the standard is
//! quoted in across published calculators, and the constants were read off
//! those rather than off the standard. The accuracy usually quoted alongside
//! them is about 5-7% for the microstrip and about 1% for the stripline. That
//! is the same position `RulesPreset::provenance` takes about the IPC rule
//! tables, and the same rule applies to a reader: check a controlled-impedance
//! stack against your fabricator's own calculator before committing an order.
//!
//! Published validity ranges, stated here rather than enforced, because the
//! symbols they are written in do not map cleanly onto these parameters:
//! `W/(H-T) < 0.35` and `T/H < 0.25` for the stripline. What **is** enforced
//! is `1 < Er < 15`, which both forms share, and that every dimension is
//! positive and the logarithm's argument is greater than one - below that the
//! formula returns zero or a negative impedance, which is not a small error
//! but a meaningless one.
//!
//! # Units
//!
//! Dimensions are [`Nm`]. `Er` is in thousandths, matching
//! `StackupLayer::dk_x1000`, and the result is in hundredths of an ohm,
//! matching `DesignConstraints::default_impedance_ohms_x100`. Neither is a
//! float, because both feed structures that are `Eq` and `Hash`.

use cypcb_core::Nm;

/// The narrowest and widest dielectric constant either form is quoted for.
const DK_X1000_MIN: u32 = 1_000;
const DK_X1000_MAX: u32 = 15_000;

/// `0.8 W + T`, the effective conductor width both forms use.
///
/// Returns `None` when either dimension is zero or negative, which is a board
/// nobody can build rather than a number worth reporting.
fn effective_width_mm(width: Nm, copper: Nm) -> Option<f64> {
    if width.raw() <= 0 || copper.raw() <= 0 {
        return None;
    }
    Some(0.8 * width.to_mm() + copper.to_mm())
}

fn dk(dk_x1000: u32) -> Option<f64> {
    if dk_x1000 <= DK_X1000_MIN || dk_x1000 >= DK_X1000_MAX {
        return None;
    }
    Some(f64::from(dk_x1000) / 1_000.0)
}

/// Round an impedance in ohms to hundredths, refusing what the formula cannot
/// answer.
fn ohms_x100(z: f64) -> Option<u32> {
    if !z.is_finite() || z <= 0.0 {
        return None;
    }
    Some((z * 100.0).round() as u32)
}

/// A trace on an outer layer over one reference plane.
///
/// `height` is the dielectric between the trace and the plane under it - on a
/// four-layer board that is the top prepreg, not the whole board.
///
/// Returns `None` when the inputs are outside what the form answers for:
/// see the module note.
pub fn microstrip_ohms_x100(width: Nm, height: Nm, copper: Nm, dk_x1000: u32) -> Option<u32> {
    let effective = effective_width_mm(width, copper)?;
    let er = dk(dk_x1000)?;
    if height.raw() <= 0 {
        return None;
    }
    let ratio = 5.98 * height.to_mm() / effective;
    if ratio <= 1.0 {
        return None;
    }
    ohms_x100(87.0 / (er + 1.41).sqrt() * ratio.ln())
}

/// A trace on an inner layer, centred between two reference planes.
///
/// `plate_separation` is the distance between the two planes, not the distance
/// from the trace to one of them.
pub fn stripline_ohms_x100(
    width: Nm,
    plate_separation: Nm,
    copper: Nm,
    dk_x1000: u32,
) -> Option<u32> {
    let effective = effective_width_mm(width, copper)?;
    let er = dk(dk_x1000)?;
    if plate_separation.raw() <= 0 {
        return None;
    }
    let ratio = 4.0 * plate_separation.to_mm() / (0.67 * std::f64::consts::PI * effective);
    if ratio <= 1.0 {
        return None;
    }
    ohms_x100(60.0 / er.sqrt() * ratio.ln())
}

/// The narrowest and widest trace this solver will look at.
///
/// A tenth of a millimetre either side of anything a fabricator images: below
/// 0.01mm nobody etches, above 10mm nobody calls it a trace. The bracket is
/// here so a target the stack cannot deliver ends as `None` rather than as a
/// search that walks off into a number no board could use.
const NARROWEST: Nm = Nm(10_000);
const WIDEST: Nm = Nm(10_000_000);

/// How close the search has to get before it stops, in nanometres.
///
/// The forms are quoted at 5-7%, so this is far below the accuracy of the
/// answer and exists only to make the search terminate on an exact number
/// rather than an asymptote. It is 100nm rather than a micrometre because a
/// stripline moves about 0.03 ohm per micrometre of width, which is visible in
/// a test that checks the solver against its own form.
const CLOSE_ENOUGH: i64 = 100;

/// The width that gives this impedance, by bisection.
///
/// Both closed forms are `k * ln(c / w)`: monotone **decreasing** in width, so
/// a wider trace is a lower impedance and there is exactly one width for a
/// reachable target. Neither inverts in closed form - the width is inside a
/// logarithm and under a correction for the foil thickness - so this searches
/// instead of solving, which is what every field solver and every fab
/// calculator does with the same equations.
///
/// `None` when the target is outside what this stack can deliver between
/// [`NARROWEST`] and [`WIDEST`], or when the form itself refuses the geometry.
fn width_for(target_x100: u32, ohms_at: impl Fn(Nm) -> Option<u32>) -> Option<Nm> {
    if target_x100 == 0 {
        return None;
    }
    // The narrow end is the high-impedance end. A form that refuses the
    // narrowest width refuses the geometry rather than the target.
    let at_narrowest = ohms_at(NARROWEST)?;
    if at_narrowest < target_x100 {
        return None;
    }
    // The wide end may fall outside the form's range, which is itself the
    // answer that the target is too low for this stack: walk in until the form
    // answers, and if it never does, say so.
    let mut low = NARROWEST;
    let mut high = WIDEST;
    let mut at_high = None;
    for _ in 0..64 {
        match ohms_at(high) {
            Some(ohms) => {
                at_high = Some(ohms);
                break;
            }
            None => {
                let next = Nm((low.raw() + high.raw()) / 2);
                if next.raw() <= low.raw() {
                    break;
                }
                high = next;
            }
        }
    }
    if at_high? > target_x100 {
        return None;
    }

    while high.raw() - low.raw() > CLOSE_ENOUGH {
        let middle = Nm((low.raw() + high.raw()) / 2);
        match ohms_at(middle) {
            // Too high an impedance means too narrow a trace.
            Some(ohms) if ohms > target_x100 => low = middle,
            Some(_) => high = middle,
            // Out of the form's range on the wide side.
            None => high = middle,
        }
    }
    Some(Nm((low.raw() + high.raw()) / 2))
}

/// The trace width that gives this impedance on an outer layer.
///
/// The inverse of [`microstrip_ohms_x100`], and the question a designer
/// actually asks: the stack is what the fabricator presses, the target is what
/// the part datasheet demands, and the width is the only thing left to choose.
///
/// # Examples
///
/// ```
/// use cypcb_calc::{microstrip_ohms_x100, microstrip_width_for_ohms_x100};
/// use cypcb_core::Nm;
///
/// let height = Nm::from_mm(0.2);
/// let copper = Nm::from_mm(0.035);
/// let width = microstrip_width_for_ohms_x100(5_000, height, copper, 4_500)
///     .expect("50 ohm is reachable on this stack");
///
/// // And the forward form agrees, to the nearest hundredth of an ohm.
/// let back = microstrip_ohms_x100(width, height, copper, 4_500).expect("in range");
/// assert!(back.abs_diff(5_000) <= 2, "{back}");
/// ```
pub fn microstrip_width_for_ohms_x100(
    target_x100: u32,
    height: Nm,
    copper: Nm,
    dk_x1000: u32,
) -> Option<Nm> {
    width_for(target_x100, |width| {
        microstrip_ohms_x100(width, height, copper, dk_x1000)
    })
}

/// The trace width that gives this impedance on a centred inner layer.
///
/// The inverse of [`stripline_ohms_x100`].
///
/// # Examples
///
/// ```
/// use cypcb_calc::{stripline_ohms_x100, stripline_width_for_ohms_x100};
/// use cypcb_core::Nm;
///
/// let separation = Nm::from_mm(0.4);
/// let copper = Nm::from_mm(0.0175);
/// let width = stripline_width_for_ohms_x100(5_000, separation, copper, 4_500)
///     .expect("50 ohm is reachable between these planes");
///
/// let back = stripline_ohms_x100(width, separation, copper, 4_500).expect("in range");
/// assert!(back.abs_diff(5_000) <= 2, "{back}");
/// ```
pub fn stripline_width_for_ohms_x100(
    target_x100: u32,
    plate_separation: Nm,
    copper: Nm,
    dk_x1000: u32,
) -> Option<Nm> {
    width_for(target_x100, |width| {
        stripline_ohms_x100(width, plate_separation, copper, dk_x1000)
    })
}
