//! Metadata components for board entities.
//!
//! These components provide source tracking and categorization
//! for error reporting and organization.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

/// Link back to source file location.
///
/// Used for error reporting and "go to definition" functionality.
/// Stores byte offsets and line/column for human-readable messages.
///
/// # Examples
///
/// ```
/// use cypcb_world::SourceSpan;
///
/// let span = SourceSpan::new(100, 150, 10, 5);
/// assert_eq!(span.start_byte, 100);
/// assert_eq!(span.end_byte, 150);
/// assert_eq!(span.start_line, 10);
/// assert_eq!(span.start_column, 5);
/// ```
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceSpan {
    /// Starting byte offset in the source file.
    pub start_byte: usize,
    /// Ending byte offset in the source file.
    pub end_byte: usize,
    /// Starting line number (1-indexed).
    pub start_line: u32,
    /// Starting column number (1-indexed).
    pub start_column: u32,
}

impl SourceSpan {
    /// Create a new source span.
    #[inline]
    pub fn new(start_byte: usize, end_byte: usize, start_line: u32, start_column: u32) -> Self {
        SourceSpan {
            start_byte,
            end_byte,
            start_line,
            start_column,
        }
    }

    /// Create a span at a single point.
    #[inline]
    pub fn point(byte: usize, line: u32, column: u32) -> Self {
        SourceSpan {
            start_byte: byte,
            end_byte: byte,
            start_line: line,
            start_column: column,
        }
    }

    /// Get the byte length of this span.
    #[inline]
    pub fn len(&self) -> usize {
        self.end_byte - self.start_byte
    }

    /// Check if this span is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start_byte == self.end_byte
    }

    /// Merge two spans to create a span covering both.
    pub fn merge(&self, other: &SourceSpan) -> SourceSpan {
        let (start_span, _) = if self.start_byte <= other.start_byte {
            (self, other)
        } else {
            (other, self)
        };

        SourceSpan {
            start_byte: self.start_byte.min(other.start_byte),
            end_byte: self.end_byte.max(other.end_byte),
            start_line: start_span.start_line,
            start_column: start_span.start_column,
        }
    }
}

impl Default for SourceSpan {
    fn default() -> Self {
        SourceSpan::new(0, 0, 1, 1)
    }
}

impl std::fmt::Display for SourceSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.start_line, self.start_column)
    }
}

/// Convert to miette::SourceSpan for error display.
impl From<SourceSpan> for miette::SourceSpan {
    fn from(span: SourceSpan) -> Self {
        (span.start_byte, span.end_byte - span.start_byte).into()
    }
}

/// Component type/kind for categorization.
///
/// Matches the component types supported in the DSL grammar.
/// Used for filtering, reporting, and BOM generation.
///
/// # Examples
///
/// ```
/// use cypcb_world::ComponentKind;
///
/// let kind = ComponentKind::Resistor;
/// assert_eq!(kind.bom_category(), "Resistors");
/// ```
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ComponentKind {
    /// Resistor (R prefix).
    Resistor,
    /// Capacitor (C prefix).
    Capacitor,
    /// Inductor (L prefix).
    Inductor,
    /// Integrated circuit (U prefix).
    IC,
    /// Light-emitting diode (LED prefix).
    LED,
    /// Connector (J/P prefix).
    Connector,
    /// Diode (D prefix).
    Diode,
    /// Transistor (Q prefix).
    Transistor,
    /// Crystal/oscillator (Y prefix).
    Crystal,
    /// Generic/other component.
    Generic,
}

impl ComponentKind {
    /// Get the standard reference designator prefix for this component type.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_world::ComponentKind;
    ///
    /// assert_eq!(ComponentKind::Resistor.refdes_prefix(), "R");
    /// assert_eq!(ComponentKind::IC.refdes_prefix(), "U");
    /// ```
    pub fn refdes_prefix(&self) -> &'static str {
        match self {
            ComponentKind::Resistor => "R",
            ComponentKind::Capacitor => "C",
            ComponentKind::Inductor => "L",
            ComponentKind::IC => "U",
            ComponentKind::LED => "LED",
            ComponentKind::Connector => "J",
            ComponentKind::Diode => "D",
            ComponentKind::Transistor => "Q",
            ComponentKind::Crystal => "Y",
            ComponentKind::Generic => "X",
        }
    }

    /// Get the BOM category name for this component type.
    pub fn bom_category(&self) -> &'static str {
        match self {
            ComponentKind::Resistor => "Resistors",
            ComponentKind::Capacitor => "Capacitors",
            ComponentKind::Inductor => "Inductors",
            ComponentKind::IC => "Integrated Circuits",
            ComponentKind::LED => "LEDs",
            ComponentKind::Connector => "Connectors",
            ComponentKind::Diode => "Diodes",
            ComponentKind::Transistor => "Transistors",
            ComponentKind::Crystal => "Crystals & Oscillators",
            ComponentKind::Generic => "Other",
        }
    }

    /// Parse a component kind from a string.
    ///
    /// Case-insensitive matching.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_world::ComponentKind;
    ///
    /// assert_eq!(ComponentKind::from_str("resistor"), Some(ComponentKind::Resistor));
    /// assert_eq!(ComponentKind::from_str("IC"), Some(ComponentKind::IC));
    /// assert_eq!(ComponentKind::from_str("unknown"), None);
    /// ```
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "resistor" => Some(ComponentKind::Resistor),
            "capacitor" | "cap" => Some(ComponentKind::Capacitor),
            "inductor" => Some(ComponentKind::Inductor),
            "ic" => Some(ComponentKind::IC),
            "led" => Some(ComponentKind::LED),
            "connector" => Some(ComponentKind::Connector),
            "diode" => Some(ComponentKind::Diode),
            "transistor" => Some(ComponentKind::Transistor),
            "crystal" | "oscillator" => Some(ComponentKind::Crystal),
            "generic" => Some(ComponentKind::Generic),
            _ => None,
        }
    }
}

impl std::fmt::Display for ComponentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComponentKind::Resistor => write!(f, "resistor"),
            ComponentKind::Capacitor => write!(f, "capacitor"),
            ComponentKind::Inductor => write!(f, "inductor"),
            ComponentKind::IC => write!(f, "ic"),
            ComponentKind::LED => write!(f, "led"),
            ComponentKind::Connector => write!(f, "connector"),
            ComponentKind::Diode => write!(f, "diode"),
            ComponentKind::Transistor => write!(f, "transistor"),
            ComponentKind::Crystal => write!(f, "crystal"),
            ComponentKind::Generic => write!(f, "generic"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_span() {
        let span = SourceSpan::new(10, 50, 5, 3);
        assert_eq!(span.len(), 40);
        assert!(!span.is_empty());
    }

    #[test]
    fn test_source_span_merge() {
        let span1 = SourceSpan::new(10, 20, 1, 10);
        let span2 = SourceSpan::new(30, 50, 2, 5);
        let merged = span1.merge(&span2);

        assert_eq!(merged.start_byte, 10);
        assert_eq!(merged.end_byte, 50);
        assert_eq!(merged.start_line, 1);
        assert_eq!(merged.start_column, 10);
    }

    #[test]
    fn test_component_kind_prefix() {
        assert_eq!(ComponentKind::Resistor.refdes_prefix(), "R");
        assert_eq!(ComponentKind::Capacitor.refdes_prefix(), "C");
        assert_eq!(ComponentKind::IC.refdes_prefix(), "U");
        assert_eq!(ComponentKind::LED.refdes_prefix(), "LED");
    }

    #[test]
    fn test_component_kind_from_str() {
        assert_eq!(ComponentKind::from_str("resistor"), Some(ComponentKind::Resistor));
        assert_eq!(ComponentKind::from_str("RESISTOR"), Some(ComponentKind::Resistor));
        assert_eq!(ComponentKind::from_str("cap"), Some(ComponentKind::Capacitor));
        assert_eq!(ComponentKind::from_str("IC"), Some(ComponentKind::IC));
        assert_eq!(ComponentKind::from_str("unknown"), None);
    }

    #[test]
    fn test_component_kind_bom_category() {
        assert_eq!(ComponentKind::Resistor.bom_category(), "Resistors");
        assert_eq!(ComponentKind::IC.bom_category(), "Integrated Circuits");
        assert_eq!(ComponentKind::Generic.bom_category(), "Other");
    }
}
