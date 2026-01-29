use crate::error::LibraryError;
use serde::{Deserialize, Serialize};

/// Pad shape enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PadShape {
    Rect,
    Circle,
    RoundRect,
    Oval,
}

/// Pad information for preview
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PadInfo {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub shape: PadShape,
}

/// Outline segment for preview
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineSegment {
    pub start_x: f64,
    pub start_y: f64,
    pub end_x: f64,
    pub end_y: f64,
    pub layer: String,
}

/// Bounding box for courtyard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

/// Footprint preview data extracted from S-expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FootprintPreview {
    pub name: String,
    pub pads: Vec<PadInfo>,
    pub outlines: Vec<OutlineSegment>,
    pub courtyard: Option<BoundingBox>,
    pub description: Option<String>,
}

/// Extract footprint preview data from KiCad S-expression
pub fn extract_preview(footprint_data: &str) -> Result<FootprintPreview, LibraryError> {
    let value = lexpr::from_str(footprint_data)
        .map_err(|e| LibraryError::Parse(format!("Invalid S-expression: {}", e)))?;

    // Extract footprint name (first string after "footprint" or "module")
    let name = extract_footprint_name(&value)?;

    // Extract description
    let description = extract_field(&value, "descr");

    // Extract pads
    let pads = extract_pads(&value);

    // Extract outline segments
    let outlines = extract_outlines(&value);

    // Extract courtyard bounding box
    let courtyard = extract_courtyard(&value);

    Ok(FootprintPreview {
        name,
        pads,
        outlines,
        courtyard,
        description,
    })
}

/// Extract footprint name from S-expression
fn extract_footprint_name(value: &lexpr::Value) -> Result<String, LibraryError> {
    use lexpr::Value;

    if let Value::Cons(cons) = value {
        if let Value::Symbol(sym) = cons.car() {
            if sym.as_ref() == "footprint" || sym.as_ref() == "module" {
                // Next element should be the name string
                if let Value::Cons(rest) = cons.cdr() {
                    if let Value::String(name) = rest.car() {
                        return Ok(name.to_string());
                    }
                }
            }
        }
    }

    Err(LibraryError::Parse("Footprint name not found".to_string()))
}

/// Extract a field value from S-expression by field name
fn extract_field(value: &lexpr::Value, field_name: &str) -> Option<String> {
    use lexpr::Value;

    // Recursive search through S-expression
    fn search(value: &Value, field_name: &str) -> Option<String> {
        match value {
            Value::Cons(cons) => {
                // Check if this is the field we're looking for
                if let Value::Symbol(sym) = cons.car() {
                    if sym.as_ref() == field_name {
                        // Next element should be the value
                        if let Value::Cons(rest) = cons.cdr() {
                            if let Value::String(val) = rest.car() {
                                return Some(val.to_string());
                            }
                        }
                    }
                }

                // Continue searching in car and cdr
                if let Some(result) = search(cons.car(), field_name) {
                    return Some(result);
                }
                if let Some(result) = search(cons.cdr(), field_name) {
                    return Some(result);
                }

                None
            }
            _ => None,
        }
    }

    search(value, field_name)
}

/// Extract pad information from S-expression
fn extract_pads(value: &lexpr::Value) -> Vec<PadInfo> {
    use lexpr::Value;

    let mut pads = Vec::new();

    fn search_pads(value: &Value, pads: &mut Vec<PadInfo>) {
        match value {
            Value::Cons(cons) => {
                // Check if this is a pad element
                if let Value::Symbol(sym) = cons.car() {
                    if sym.as_ref() == "pad" {
                        if let Some(pad) = parse_pad(cons) {
                            pads.push(pad);
                        }
                    }
                }

                // Continue searching recursively
                search_pads(cons.car(), pads);
                search_pads(cons.cdr(), pads);
            }
            _ => {}
        }
    }

    search_pads(value, &mut pads);
    pads
}

/// Parse a single pad element
fn parse_pad(cons: &lexpr::Cons) -> Option<PadInfo> {
    use lexpr::Value;

    // Format: (pad "1" smd rect (at -1 0) (size 1 0.95) ...)
    let mut current = cons.cdr();

    // Extract pad name (first string after "pad")
    let name = if let Value::Cons(c) = current {
        if let Value::String(n) = c.car() {
            current = c.cdr();
            n.to_string()
        } else {
            return None;
        }
    } else {
        return None;
    };

    // Extract pad type (smd, thru_hole, etc.) - skip for now
    if let Value::Cons(c) = current {
        current = c.cdr();
    }

    // Extract shape
    let shape = if let Value::Cons(c) = current {
        if let Value::Symbol(s) = c.car() {
            let shape_str = s.as_ref();
            current = c.cdr();
            match shape_str {
                "rect" => PadShape::Rect,
                "circle" => PadShape::Circle,
                "roundrect" => PadShape::RoundRect,
                "oval" => PadShape::Oval,
                _ => PadShape::Rect,
            }
        } else {
            PadShape::Rect
        }
    } else {
        PadShape::Rect
    };

    // Extract position (at x y)
    let (x, y) = extract_at(current)?;

    // Extract size (size w h)
    let (width, height) = extract_size(current)?;

    Some(PadInfo {
        name,
        x,
        y,
        width,
        height,
        shape,
    })
}

/// Extract position from (at x y) element
fn extract_at(value: &lexpr::Value) -> Option<(f64, f64)> {
    use lexpr::Value;

    fn search_at(value: &Value) -> Option<(f64, f64)> {
        match value {
            Value::Cons(cons) => {
                if let Value::Symbol(sym) = cons.car() {
                    if sym.as_ref() == "at" {
                        // Next should be x, then y
                        if let Value::Cons(rest1) = cons.cdr() {
                            let x = parse_number(rest1.car())?;
                            if let Value::Cons(rest2) = rest1.cdr() {
                                let y = parse_number(rest2.car())?;
                                return Some((x, y));
                            }
                        }
                    }
                }

                // Search recursively
                if let Some(result) = search_at(cons.car()) {
                    return Some(result);
                }
                if let Some(result) = search_at(cons.cdr()) {
                    return Some(result);
                }
                None
            }
            _ => None,
        }
    }

    search_at(value)
}

/// Extract size from (size w h) element
fn extract_size(value: &lexpr::Value) -> Option<(f64, f64)> {
    use lexpr::Value;

    fn search_size(value: &Value) -> Option<(f64, f64)> {
        match value {
            Value::Cons(cons) => {
                if let Value::Symbol(sym) = cons.car() {
                    if sym.as_ref() == "size" {
                        // Next should be width, then height
                        if let Value::Cons(rest1) = cons.cdr() {
                            let w = parse_number(rest1.car())?;
                            if let Value::Cons(rest2) = rest1.cdr() {
                                let h = parse_number(rest2.car())?;
                                return Some((w, h));
                            }
                        }
                    }
                }

                // Search recursively
                if let Some(result) = search_size(cons.car()) {
                    return Some(result);
                }
                if let Some(result) = search_size(cons.cdr()) {
                    return Some(result);
                }
                None
            }
            _ => None,
        }
    }

    search_size(value)
}

/// Parse a number from lexpr Value
fn parse_number(value: &lexpr::Value) -> Option<f64> {
    use lexpr::Value;

    match value {
        Value::Number(num) => {
            if let Some(i) = num.as_i64() {
                Some(i as f64)
            } else if let Some(f) = num.as_f64() {
                Some(f)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Extract outline segments (fp_line on F.SilkS or F.Fab)
fn extract_outlines(value: &lexpr::Value) -> Vec<OutlineSegment> {
    use lexpr::Value;

    let mut outlines = Vec::new();

    fn search_lines(value: &Value, outlines: &mut Vec<OutlineSegment>) {
        match value {
            Value::Cons(cons) => {
                if let Value::Symbol(sym) = cons.car() {
                    if sym.as_ref() == "fp_line" {
                        if let Some(line) = parse_fp_line(cons) {
                            // Only include silkscreen and fab layers
                            if line.layer == "F.SilkS" || line.layer == "F.Fab" {
                                outlines.push(line);
                            }
                        }
                    }
                }

                search_lines(cons.car(), outlines);
                search_lines(cons.cdr(), outlines);
            }
            _ => {}
        }
    }

    search_lines(value, &mut outlines);
    outlines
}

/// Parse fp_line element
fn parse_fp_line(cons: &lexpr::Cons) -> Option<OutlineSegment> {
    // Format: (fp_line (start x1 y1) (end x2 y2) (layer "F.SilkS") ...)
    let current = cons.cdr();

    // Find start, end, and layer
    let (start_x, start_y) = extract_start(current)?;
    let (end_x, end_y) = extract_end(current)?;
    let layer = extract_layer(current)?;

    Some(OutlineSegment {
        start_x,
        start_y,
        end_x,
        end_y,
        layer,
    })
}

/// Extract start position from fp_line
fn extract_start(value: &lexpr::Value) -> Option<(f64, f64)> {
    use lexpr::Value;

    fn search_start(value: &Value) -> Option<(f64, f64)> {
        match value {
            Value::Cons(cons) => {
                if let Value::Symbol(sym) = cons.car() {
                    if sym.as_ref() == "start" {
                        if let Value::Cons(rest1) = cons.cdr() {
                            let x = parse_number(rest1.car())?;
                            if let Value::Cons(rest2) = rest1.cdr() {
                                let y = parse_number(rest2.car())?;
                                return Some((x, y));
                            }
                        }
                    }
                }

                if let Some(result) = search_start(cons.car()) {
                    return Some(result);
                }
                if let Some(result) = search_start(cons.cdr()) {
                    return Some(result);
                }
                None
            }
            _ => None,
        }
    }

    search_start(value)
}

/// Extract end position from fp_line
fn extract_end(value: &lexpr::Value) -> Option<(f64, f64)> {
    use lexpr::Value;

    fn search_end(value: &Value) -> Option<(f64, f64)> {
        match value {
            Value::Cons(cons) => {
                if let Value::Symbol(sym) = cons.car() {
                    if sym.as_ref() == "end" {
                        if let Value::Cons(rest1) = cons.cdr() {
                            let x = parse_number(rest1.car())?;
                            if let Value::Cons(rest2) = rest1.cdr() {
                                let y = parse_number(rest2.car())?;
                                return Some((x, y));
                            }
                        }
                    }
                }

                if let Some(result) = search_end(cons.car()) {
                    return Some(result);
                }
                if let Some(result) = search_end(cons.cdr()) {
                    return Some(result);
                }
                None
            }
            _ => None,
        }
    }

    search_end(value)
}

/// Extract layer from element
fn extract_layer(value: &lexpr::Value) -> Option<String> {
    extract_field(value, "layer")
}

/// Extract courtyard bounding box from F.CrtYd layer
fn extract_courtyard(value: &lexpr::Value) -> Option<BoundingBox> {
    use lexpr::Value;

    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;
    let mut found_courtyard = false;

    fn search_courtyard(
        value: &Value,
        min_x: &mut f64,
        min_y: &mut f64,
        max_x: &mut f64,
        max_y: &mut f64,
        found: &mut bool,
    ) {
        match value {
            Value::Cons(cons) => {
                if let Value::Symbol(sym) = cons.car() {
                    if sym.as_ref() == "fp_line" {
                        if let Some(line) = parse_fp_line(cons) {
                            if line.layer == "F.CrtYd" {
                                *found = true;
                                *min_x = min_x.min(line.start_x).min(line.end_x);
                                *min_y = min_y.min(line.start_y).min(line.end_y);
                                *max_x = max_x.max(line.start_x).max(line.end_x);
                                *max_y = max_y.max(line.start_y).max(line.end_y);
                            }
                        }
                    }
                }

                search_courtyard(cons.car(), min_x, min_y, max_x, max_y, found);
                search_courtyard(cons.cdr(), min_x, min_y, max_x, max_y, found);
            }
            _ => {}
        }
    }

    search_courtyard(value, &mut min_x, &mut min_y, &mut max_x, &mut max_y, &mut found_courtyard);

    if found_courtyard {
        Some(BoundingBox {
            min_x,
            min_y,
            max_x,
            max_y,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_preview_basic() {
        let sexpr = r#"(footprint "R_0805_2012Metric"
  (version 20211014)
  (generator pcbnew)
  (layer "F.Cu")
  (descr "Resistor SMD 0805")
  (pad "1" smd rect (at -1 0) (size 1 0.95) (layers "F.Cu" "F.Paste" "F.Mask"))
  (pad "2" smd rect (at 1 0) (size 1 0.95) (layers "F.Cu" "F.Paste" "F.Mask"))
)"#;

        let preview = extract_preview(sexpr).unwrap();

        assert_eq!(preview.name, "R_0805_2012Metric");
        assert_eq!(preview.description, Some("Resistor SMD 0805".to_string()));
        assert_eq!(preview.pads.len(), 2);

        // Check first pad
        assert_eq!(preview.pads[0].name, "1");
        assert_eq!(preview.pads[0].x, -1.0);
        assert_eq!(preview.pads[0].y, 0.0);
        assert_eq!(preview.pads[0].width, 1.0);
        assert_eq!(preview.pads[0].height, 0.95);
        assert_eq!(preview.pads[0].shape, PadShape::Rect);

        // Check second pad
        assert_eq!(preview.pads[1].name, "2");
        assert_eq!(preview.pads[1].x, 1.0);
        assert_eq!(preview.pads[1].y, 0.0);
    }

    #[test]
    fn test_extract_preview_with_outlines() {
        let sexpr = r#"(footprint "TestComponent"
  (descr "Test")
  (fp_line (start -1.5 -1) (end 1.5 -1) (layer "F.SilkS") (width 0.12))
  (fp_line (start -1.5 1) (end 1.5 1) (layer "F.SilkS") (width 0.12))
  (pad "1" smd rect (at 0 0) (size 0.8 0.8))
)"#;

        let preview = extract_preview(sexpr).unwrap();

        assert_eq!(preview.name, "TestComponent");
        assert_eq!(preview.outlines.len(), 2);

        // Check first outline
        assert_eq!(preview.outlines[0].start_x, -1.5);
        assert_eq!(preview.outlines[0].start_y, -1.0);
        assert_eq!(preview.outlines[0].end_x, 1.5);
        assert_eq!(preview.outlines[0].end_y, -1.0);
        assert_eq!(preview.outlines[0].layer, "F.SilkS");
    }

    #[test]
    fn test_extract_preview_with_courtyard() {
        let sexpr = r#"(footprint "TestComponent"
  (fp_line (start -2 -1.5) (end 2 -1.5) (layer "F.CrtYd") (width 0.05))
  (fp_line (start -2 1.5) (end 2 1.5) (layer "F.CrtYd") (width 0.05))
  (pad "1" smd rect (at 0 0) (size 0.8 0.8))
)"#;

        let preview = extract_preview(sexpr).unwrap();

        assert!(preview.courtyard.is_some());
        let courtyard = preview.courtyard.unwrap();
        assert_eq!(courtyard.min_x, -2.0);
        assert_eq!(courtyard.min_y, -1.5);
        assert_eq!(courtyard.max_x, 2.0);
        assert_eq!(courtyard.max_y, 1.5);
    }

    #[test]
    fn test_extract_preview_missing_footprint_data() {
        let result = extract_preview("");
        assert!(result.is_err());
    }
}
