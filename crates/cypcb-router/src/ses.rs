//! Specctra SES (Session) file import.
//!
//! Imports routing results from FreeRouting's SES output format.
//!
//! # SES Format Overview
//!
//! The Specctra SES format contains routing results in S-expression syntax:
//!
//! ```text
//! (session "board_name"
//!   (routes
//!     (network_out
//!       (net "VCC"
//!         (wire
//!           (path F.Cu 8 1000 2000 1500 2000)
//!         )
//!         (via via1 1500 2000)
//!       )
//!     )
//!   )
//! )
//! ```
//!
//! # Coordinate System
//!
//! - SES uses mils (thousandths of an inch)
//! - Our internal model uses nanometers
//! - Conversion: 1 mil = 25,400 nm

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use cypcb_core::{Nm, Point};
use cypcb_world::{Layer, NetId};
use thiserror::Error;

use crate::types::{RouteSegment, RoutingResult, RoutingStatus, ViaPlacement};

/// Errors that can occur during SES import.
#[derive(Debug, Error)]
pub enum SesImportError {
    /// IO error reading SES file.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Parse error in SES content.
    #[error("Parse error: {0}")]
    ParseError(String),

    /// No routes found in SES file.
    #[error("No routes found in SES file")]
    NoRoutesFound,

    /// Coordinate mismatch (internal consistency check).
    #[error("Coordinate mismatch: {0}")]
    CoordinateMismatch(String),
}

/// Convert tenth-mils to nanometers.
///
/// SES files use `(resolution mil 10)` which means 1 unit = 1/10 mil = 0.1 mil.
/// 1 tenth-mil = 25,400 / 10 = 2,540 nanometers
///
/// FreeRouting reads DSN coordinates (in mils) and outputs SES coordinates
/// scaled by 10 (in tenth-mils). For example, a pad at 196.85 mils in the DSN
/// appears as 1969 in the SES file.
#[inline]
fn tenth_mil_to_nm(tenth_mil: f64) -> Nm {
    Nm((tenth_mil * 2_540.0) as i64)
}

/// Parse a DSN/SES layer name to Layer enum.
fn parse_layer(name: &str) -> Option<Layer> {
    match name {
        "F.Cu" | "Top" | "top" => Some(Layer::TopCopper),
        "B.Cu" | "Bottom" | "bottom" => Some(Layer::BottomCopper),
        "In1.Cu" => Some(Layer::Inner(0)),
        "In2.Cu" => Some(Layer::Inner(1)),
        "In3.Cu" => Some(Layer::Inner(2)),
        "In4.Cu" => Some(Layer::Inner(3)),
        _ => None,
    }
}

/// Import a Specctra SES file and convert to RoutingResult.
///
/// # Arguments
///
/// * `path` - Path to the SES file
/// * `net_lookup` - Map from net name to NetId
///
/// # Returns
///
/// A `RoutingResult` containing all extracted routes and vias.
///
/// # Errors
///
/// Returns `SesImportError` if:
/// - File cannot be read
/// - Content cannot be parsed
/// - No routes are found
pub fn import_ses(
    path: &Path,
    net_lookup: &HashMap<String, NetId>,
) -> Result<RoutingResult, SesImportError> {
    let content = fs::read_to_string(path)?;
    import_ses_from_str(&content, net_lookup)
}

/// Import SES content from a string.
///
/// This is the main parsing function, separated for testing.
pub fn import_ses_from_str(
    content: &str,
    net_lookup: &HashMap<String, NetId>,
) -> Result<RoutingResult, SesImportError> {
    let mut routes = Vec::new();
    let mut vias = Vec::new();

    // Find the routes section
    let routes_section = find_section(content, "routes")
        .ok_or(SesImportError::NoRoutesFound)?;

    // Find network_out within routes
    let network_out = find_section(routes_section, "network_out")
        .unwrap_or(routes_section); // Some SES files have routes directly

    // Parse each net's routing
    let net_sections = find_all_sections(network_out, "net");

    if net_sections.is_empty() {
        return Ok(RoutingResult::default());
    }

    for net_section in net_sections {
        // Extract net name
        let net_name = extract_first_string(net_section)
            .ok_or_else(|| SesImportError::ParseError("Missing net name".into()))?;

        let net_id = net_lookup
            .get(&net_name)
            .copied()
            .unwrap_or(NetId::new(0)); // Default if not found

        // Parse wires in this net
        let wire_sections = find_all_sections(net_section, "wire");
        for wire_section in wire_sections {
            let segments = parse_wire(wire_section, net_id)?;
            routes.extend(segments);
        }

        // Parse vias in this net
        let via_sections = find_all_sections(net_section, "via");
        for via_section in via_sections {
            if let Some(via) = parse_via(via_section, net_id)? {
                vias.push(via);
            }
        }
    }

    let status = if routes.is_empty() && vias.is_empty() {
        RoutingStatus::Partial { unrouted_count: 0 }
    } else {
        RoutingStatus::Complete
    };

    Ok(RoutingResult {
        status,
        routes,
        vias,
    })
}

/// Parse a wire section into route segments.
///
/// Wire format: (wire (path layer width x1 y1 x2 y2 ...) [net "name"] [type ...])
fn parse_wire(section: &str, net_id: NetId) -> Result<Vec<RouteSegment>, SesImportError> {
    let mut segments = Vec::new();

    // Find path within wire
    if let Some(path_section) = find_section(section, "path") {
        let tokens: Vec<&str> = tokenize_path(path_section);

        if tokens.len() < 5 {
            return Err(SesImportError::ParseError(
                format!("Invalid path: not enough tokens (got {})", tokens.len()),
            ));
        }

        // Parse layer name (first token)
        let layer = parse_layer(tokens[0])
            .ok_or_else(|| SesImportError::ParseError(format!("Unknown layer: {}", tokens[0])))?;

        // Parse width (second token)
        let width: f64 = tokens[1]
            .parse()
            .map_err(|_| SesImportError::ParseError(format!("Invalid width: {}", tokens[1])))?;
        let width_nm = tenth_mil_to_nm(width);

        // Parse coordinate pairs (remaining tokens)
        let coords: Vec<f64> = tokens[2..]
            .iter()
            .filter_map(|s| s.parse().ok())
            .collect();

        // Convert to points
        let points: Vec<Point> = coords
            .chunks(2)
            .filter_map(|chunk| {
                if chunk.len() == 2 {
                    Some(Point::new(tenth_mil_to_nm(chunk[0]), tenth_mil_to_nm(chunk[1])))
                } else {
                    None
                }
            })
            .collect();

        // Create segments from consecutive point pairs
        for window in points.windows(2) {
            segments.push(RouteSegment::new(
                net_id,
                layer,
                width_nm,
                window[0],
                window[1],
            ));
        }
    }

    Ok(segments)
}

/// Parse a via section into a ViaPlacement.
///
/// Via format: (via padstack_name x y) or (via (type padstack_name) x y)
fn parse_via(section: &str, net_id: NetId) -> Result<Option<ViaPlacement>, SesImportError> {
    let tokens: Vec<&str> = tokenize_simple(section);

    if tokens.is_empty() {
        return Ok(None);
    }

    // Find coordinates - look for numeric values
    let mut x_mil: Option<f64> = None;
    let mut y_mil: Option<f64> = None;

    for token in tokens.iter().skip(1) {
        // Skip the first token (padstack name or type)
        if let Ok(val) = token.parse::<f64>() {
            if x_mil.is_none() {
                x_mil = Some(val);
            } else if y_mil.is_none() {
                y_mil = Some(val);
                break;
            }
        }
    }

    match (x_mil, y_mil) {
        (Some(x), Some(y)) => {
            let position = Point::new(tenth_mil_to_nm(x), tenth_mil_to_nm(y));

            // Default drill size and layers for through-hole via
            // (In a full implementation, we'd look up the padstack definition)
            let drill = Nm::from_mm(0.3); // Default 0.3mm drill

            Ok(Some(ViaPlacement::through_hole(net_id, position, drill)))
        }
        _ => Ok(None),
    }
}

/// Find a section by name in S-expression content.
///
/// Returns the content inside the parentheses.
fn find_section<'a>(content: &'a str, name: &str) -> Option<&'a str> {
    // Look for (name ...)
    let pattern = format!("({}", name);
    let start = content.find(&pattern)?;

    // Find matching closing paren
    let section_start = start + 1; // Skip opening paren
    let mut depth = 1;
    let mut end = section_start;

    for (i, c) in content[section_start..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = section_start + i;
                    break;
                }
            }
            _ => {}
        }
    }

    Some(&content[section_start..end])
}

/// Find all sections with a given name.
fn find_all_sections<'a>(content: &'a str, name: &str) -> Vec<&'a str> {
    let mut sections = Vec::new();
    let pattern = format!("({}", name);
    let mut search_start = 0;

    while let Some(rel_start) = content[search_start..].find(&pattern) {
        let abs_start = search_start + rel_start;

        // Verify this is a complete match (followed by space, quote, or paren)
        let after_pattern = abs_start + pattern.len();
        if after_pattern < content.len() {
            let next_char = content[after_pattern..].chars().next().unwrap_or(' ');
            if !next_char.is_whitespace() && next_char != '"' && next_char != '(' && next_char != ')' {
                // This is a partial match (e.g., "network_out" when searching for "net")
                search_start = abs_start + 1;
                continue;
            }
        }

        let section_start = abs_start + 1; // Skip opening paren

        // Find matching closing paren
        let mut depth = 1;
        let mut end = section_start;

        for (i, c) in content[section_start..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = section_start + i;
                        break;
                    }
                }
                _ => {}
            }
        }

        sections.push(&content[section_start..end]);
        search_start = end + 1;

        if search_start >= content.len() {
            break;
        }
    }

    sections
}

/// Extract the first quoted string from a section.
fn extract_first_string(content: &str) -> Option<String> {
    // Look for quoted string
    if let Some(start) = content.find('"') {
        let rest = &content[start + 1..];
        if let Some(end) = rest.find('"') {
            return Some(rest[..end].to_string());
        }
    }

    // Fall back to first space-delimited token after section name
    let tokens: Vec<&str> = content.split_whitespace().collect();
    if tokens.len() >= 2 {
        // Remove any surrounding quotes
        let token = tokens[1].trim_matches('"');
        return Some(token.to_string());
    }

    None
}

/// Tokenize a path section (layer width x1 y1 x2 y2 ...).
fn tokenize_path(content: &str) -> Vec<&str> {
    // Skip "path" keyword and collect tokens
    content
        .split_whitespace()
        .skip(1) // Skip "path"
        .filter(|s| !s.is_empty() && !s.starts_with('(') && !s.ends_with(')'))
        .map(|s| s.trim_matches(|c| c == '(' || c == ')'))
        .collect()
}

/// Simple tokenization for via and other sections.
fn tokenize_simple(content: &str) -> Vec<&str> {
    content
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .map(|s| s.trim_matches(|c| c == '(' || c == ')' || c == '"'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_net_lookup() -> HashMap<String, NetId> {
        let mut map = HashMap::new();
        map.insert("VCC".to_string(), NetId::new(1));
        map.insert("GND".to_string(), NetId::new(2));
        map.insert("SIG".to_string(), NetId::new(3));
        map
    }

    #[test]
    fn test_tenth_mil_to_nm() {
        // 1 tenth-mil = 2540 nm  (0.1 mil * 25400 nm/mil)
        assert_eq!(tenth_mil_to_nm(1.0), Nm(2_540));
        assert_eq!(tenth_mil_to_nm(10.0), Nm(25_400));

        // 10 tenth-mils = 1 mil = 25400 nm
        assert_eq!(tenth_mil_to_nm(10.0), Nm(25_400));

        // Round-trip: 1mm = 1_000_000 nm = ~393.7 tenth-mils
        // DSN exports mils (nm/25400), FreeRouting outputs SES in tenth-mils (*10)
        let one_mm_nm = 1_000_000i64;
        let tenth_mils = (one_mm_nm as f64 / 25_400.0) * 10.0; // ~393.7
        assert!((tenth_mil_to_nm(tenth_mils).0 - one_mm_nm).abs() < 10);
    }

    #[test]
    fn test_parse_layer() {
        assert_eq!(parse_layer("F.Cu"), Some(Layer::TopCopper));
        assert_eq!(parse_layer("Top"), Some(Layer::TopCopper));
        assert_eq!(parse_layer("B.Cu"), Some(Layer::BottomCopper));
        assert_eq!(parse_layer("Bottom"), Some(Layer::BottomCopper));
        assert_eq!(parse_layer("In1.Cu"), Some(Layer::Inner(0)));
        assert_eq!(parse_layer("In2.Cu"), Some(Layer::Inner(1)));
        assert_eq!(parse_layer("Unknown"), None);
    }

    #[test]
    fn test_find_section() {
        let content = r#"(session "board"
            (routes
                (network_out
                    (net "VCC")
                )
            )
        )"#;

        let session = find_section(content, "session");
        assert!(session.is_some());
        assert!(session.unwrap().contains("routes"));

        let routes = find_section(content, "routes");
        assert!(routes.is_some());
        assert!(routes.unwrap().contains("network_out"));

        let missing = find_section(content, "missing");
        assert!(missing.is_none());
    }

    #[test]
    fn test_find_all_sections() {
        let content = r#"(network_out
            (net "VCC" (wire (path F.Cu 8 0 0 100 0)))
            (net "GND" (wire (path B.Cu 8 0 50 100 50)))
        )"#;

        let nets = find_all_sections(content, "net");
        assert_eq!(nets.len(), 2);
        assert!(nets[0].contains("VCC"));
        assert!(nets[1].contains("GND"));
    }

    #[test]
    fn test_extract_first_string() {
        assert_eq!(
            extract_first_string(r#"net "VCC" (pins R1-1)"#),
            Some("VCC".to_string())
        );
        assert_eq!(
            extract_first_string(r#"net GND (pins R1-2)"#),
            Some("GND".to_string())
        );
    }

    #[test]
    fn test_parse_minimal_ses() {
        // Coordinates are in tenth-mils (SES resolution mil 10 = 1 unit = 0.1 mil)
        // 10000 tenth-mils = 1000 mils = 25.4mm
        let ses = r#"(session "board"
            (routes
                (resolution mil 10)
                (network_out
                    (net "VCC"
                        (wire
                            (path F.Cu 80 10000 20000 15000 20000)
                        )
                    )
                )
            )
        )"#;

        let net_lookup = make_net_lookup();
        let result = import_ses_from_str(ses, &net_lookup).unwrap();

        assert!(result.is_complete());
        assert_eq!(result.routes.len(), 1);
        assert_eq!(result.vias.len(), 0);

        let route = &result.routes[0];
        assert_eq!(route.net_id, NetId::new(1)); // VCC
        assert_eq!(route.layer, Layer::TopCopper);
        // 80 tenth-mils * 2540 = 203_200 nm = 0.2032mm ≈ 8 mils (0.2mm)
        assert_eq!(route.width, tenth_mil_to_nm(80.0));
        assert_eq!(route.start, Point::new(tenth_mil_to_nm(10000.0), tenth_mil_to_nm(20000.0)));
        assert_eq!(route.end, Point::new(tenth_mil_to_nm(15000.0), tenth_mil_to_nm(20000.0)));
    }

    #[test]
    fn test_parse_ses_blink_coordinates() {
        // Regression test: verifies that actual blink.ses coordinates convert correctly.
        // J1 in DSN is at 196.8504 mils. In SES (resolution mil 10): 196.8504 * 10 = 1969.
        // J1 pin 1 offset: -50 mils → absolute 146.85 mils → SES: 1469 tenth-mils.
        // Expected nm: 146.9 mils * 25400 = 3,731,260 nm ≈ 3.73mm.
        let j1_pin1_tenth_mils = 1469.0_f64;
        let expected_nm = Nm((146.9 * 25_400.0) as i64);
        let result_nm = tenth_mil_to_nm(j1_pin1_tenth_mils);
        // Allow 1000 nm tolerance for rounding
        assert!((result_nm.0 - expected_nm.0).abs() < 1000,
            "J1 pin1: got {}nm, expected {}nm", result_nm.0, expected_nm.0);

        // Width: 80 tenth-mils = 8 mils = 203_200 nm (0.2mm trace width)
        assert_eq!(tenth_mil_to_nm(80.0), Nm(203_200));
    }

    #[test]
    fn test_parse_ses_with_via() {
        // Coordinates in tenth-mils (resolution mil 10)
        let ses = r#"(session "board"
            (routes
                (resolution mil 10)
                (network_out
                    (net "VCC"
                        (wire (path F.Cu 80 0 0 1000 0))
                        (via via1 1000 0)
                        (wire (path B.Cu 80 1000 0 2000 0))
                    )
                )
            )
        )"#;

        let net_lookup = make_net_lookup();
        let result = import_ses_from_str(ses, &net_lookup).unwrap();

        assert!(result.is_complete());
        assert_eq!(result.routes.len(), 2); // Two wire segments
        assert_eq!(result.vias.len(), 1);

        let via = &result.vias[0];
        assert_eq!(via.net_id, NetId::new(1)); // VCC
        assert_eq!(via.position, Point::new(tenth_mil_to_nm(1000.0), tenth_mil_to_nm(0.0)));
        assert_eq!(via.start_layer, Layer::TopCopper);
        assert_eq!(via.end_layer, Layer::BottomCopper);
    }

    #[test]
    fn test_parse_ses_with_multiple_nets() {
        // Coordinates in tenth-mils (resolution mil 10)
        let ses = r#"(session "board"
            (routes
                (resolution mil 10)
                (network_out
                    (net "VCC"
                        (wire (path F.Cu 80 0 0 1000 0))
                    )
                    (net "GND"
                        (wire (path B.Cu 100 500 500 1500 500))
                    )
                    (net "SIG"
                        (wire (path F.Cu 60 0 1000 1000 1000 1000 2000))
                    )
                )
            )
        )"#;

        let net_lookup = make_net_lookup();
        let result = import_ses_from_str(ses, &net_lookup).unwrap();

        assert!(result.is_complete());
        // VCC: 1 segment, GND: 1 segment, SIG: 2 segments (3 points = 2 segments)
        assert_eq!(result.routes.len(), 4);

        // Verify different nets
        let vcc_routes: Vec<_> = result.routes.iter().filter(|r| r.net_id == NetId::new(1)).collect();
        let gnd_routes: Vec<_> = result.routes.iter().filter(|r| r.net_id == NetId::new(2)).collect();
        let sig_routes: Vec<_> = result.routes.iter().filter(|r| r.net_id == NetId::new(3)).collect();

        assert_eq!(vcc_routes.len(), 1);
        assert_eq!(gnd_routes.len(), 1);
        assert_eq!(sig_routes.len(), 2);
    }

    #[test]
    fn test_parse_empty_routes() {
        let ses = r#"(session "board"
            (routes
                (network_out)
            )
        )"#;

        let net_lookup = make_net_lookup();
        let result = import_ses_from_str(ses, &net_lookup).unwrap();

        // Empty routes should still be valid
        assert!(result.routes.is_empty());
        assert!(result.vias.is_empty());
    }

    #[test]
    fn test_parse_no_routes_section() {
        let ses = r#"(session "board"
            (history)
        )"#;

        let net_lookup = make_net_lookup();
        let result = import_ses_from_str(ses, &net_lookup);

        assert!(matches!(result, Err(SesImportError::NoRoutesFound)));
    }

    #[test]
    fn test_coordinate_round_trip() {
        // Verify the full DSN→FreeRouting→SES coordinate round-trip:
        // 1. DSN export: nm → mils (divide by 25400)
        // 2. FreeRouting reads mils, outputs SES in tenth-mils (multiply by 10)
        // 3. SES import: tenth-mils → nm (multiply by 2540)
        //
        // Net result: nm → nm with small rounding error

        let original_nm = 1_000_000i64; // 1mm
        let as_mils = original_nm as f64 / 25_400.0; // 39.3701 mils (written to DSN)
        let as_tenth_mils = as_mils * 10.0;           // 393.701 (FreeRouting SES output)
        let back_to_nm = tenth_mil_to_nm(as_tenth_mils); // 393.701 * 2540 = 999,999.54 nm

        // Should be within 10nm (rounding error from the 0.1-mil quantization)
        assert!((back_to_nm.0 - original_nm).abs() < 10,
            "Round-trip failed: original {}nm, got {}nm", original_nm, back_to_nm.0);
    }

    #[test]
    fn test_wire_with_multiple_points() {
        // Wire with 4 points = 3 segments, coordinates in tenth-mils
        let ses = r#"(session "board"
            (routes
                (resolution mil 10)
                (network_out
                    (net "SIG"
                        (wire (path F.Cu 80 0 0 1000 0 1000 1000 2000 1000))
                    )
                )
            )
        )"#;

        let net_lookup = make_net_lookup();
        let result = import_ses_from_str(ses, &net_lookup).unwrap();

        assert_eq!(result.routes.len(), 3);

        // Check segments form a connected path
        assert_eq!(result.routes[0].start, Point::new(Nm(0), Nm(0)));
        assert_eq!(result.routes[0].end, Point::new(tenth_mil_to_nm(1000.0), Nm(0)));
        assert_eq!(result.routes[1].start, result.routes[0].end);
        assert_eq!(result.routes[2].start, result.routes[1].end);
    }
}
