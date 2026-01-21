//! Electrical components for board entities.
//!
//! These components represent the electrical properties of PCB elements:
//! net connections, reference designators, component values, and pin mappings.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

/// Unique identifier for a net (interned from name).
///
/// Net names are interned to integer IDs at parse time for efficient
/// comparison and storage. The mapping is maintained by a NetRegistry.
///
/// # Examples
///
/// ```
/// use cypcb_world::NetId;
///
/// let vcc = NetId(0);
/// let gnd = NetId(1);
/// assert_ne!(vcc, gnd);
/// ```
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct NetId(pub u32);

impl NetId {
    /// Create a new NetId.
    #[inline]
    pub const fn new(id: u32) -> Self {
        NetId(id)
    }

    /// Get the raw ID value.
    #[inline]
    pub const fn id(&self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for NetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Net({})", self.0)
    }
}

/// Reference designator for a component (R1, C1, U1, etc.).
///
/// Reference designators uniquely identify components on a board.
/// They follow standard conventions:
/// - R: Resistors
/// - C: Capacitors
/// - L: Inductors
/// - U: ICs
/// - LED: LEDs
/// - J/P: Connectors
///
/// # Examples
///
/// ```
/// use cypcb_world::RefDes;
///
/// let r1 = RefDes::new("R1");
/// assert_eq!(r1.as_str(), "R1");
/// ```
#[derive(Component, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RefDes(pub String);

impl RefDes {
    /// Create a new reference designator.
    #[inline]
    pub fn new(refdes: impl Into<String>) -> Self {
        RefDes(refdes.into())
    }

    /// Get the reference designator as a string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Extract the prefix (letters) from the reference designator.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_world::RefDes;
    ///
    /// assert_eq!(RefDes::new("R1").prefix(), "R");
    /// assert_eq!(RefDes::new("LED2").prefix(), "LED");
    /// assert_eq!(RefDes::new("U100").prefix(), "U");
    /// ```
    pub fn prefix(&self) -> &str {
        let end = self.0.find(|c: char| c.is_ascii_digit()).unwrap_or(self.0.len());
        &self.0[..end]
    }

    /// Extract the number suffix from the reference designator.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_world::RefDes;
    ///
    /// assert_eq!(RefDes::new("R1").number(), Some(1));
    /// assert_eq!(RefDes::new("LED2").number(), Some(2));
    /// assert_eq!(RefDes::new("U100").number(), Some(100));
    /// assert_eq!(RefDes::new("TEST").number(), None);
    /// ```
    pub fn number(&self) -> Option<u32> {
        let start = self.0.find(|c: char| c.is_ascii_digit())?;
        self.0[start..].parse().ok()
    }
}

impl std::fmt::Display for RefDes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for RefDes {
    fn from(s: &str) -> Self {
        RefDes::new(s)
    }
}

impl From<String> for RefDes {
    fn from(s: String) -> Self {
        RefDes(s)
    }
}

/// Component value (10k, 100nF, ATmega328P).
///
/// Stores the value string for a component. This can be:
/// - Resistance values: "10k", "4.7k", "1M"
/// - Capacitance values: "100nF", "10uF", "1pF"
/// - IC part numbers: "ATmega328P", "STM32F103"
/// - Any other identifying string
///
/// # Examples
///
/// ```
/// use cypcb_world::Value;
///
/// let resistor_value = Value::new("10k");
/// let cap_value = Value::new("100nF");
/// let ic_value = Value::new("ATmega328P");
/// ```
#[derive(Component, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Value(pub String);

impl Value {
    /// Create a new value.
    #[inline]
    pub fn new(value: impl Into<String>) -> Self {
        Value(value.into())
    }

    /// Get the value as a string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::new(s)
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value(s)
    }
}

/// A single pin-to-net connection.
///
/// Maps a pin identifier to a net ID. Pin identifiers can be:
/// - Numbers: "1", "2" (for simple components)
/// - Names: "VCC", "GND", "anode", "cathode"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PinConnection {
    /// The pin identifier (number or name).
    pub pin: String,
    /// The net this pin connects to.
    pub net: NetId,
}

impl PinConnection {
    /// Create a new pin connection.
    #[inline]
    pub fn new(pin: impl Into<String>, net: NetId) -> Self {
        PinConnection {
            pin: pin.into(),
            net,
        }
    }
}

/// Collection of pin-to-net connections for a component.
///
/// Stores all the net connections for a single component.
/// Used to determine which pins connect to which nets.
///
/// # Examples
///
/// ```
/// use cypcb_world::{NetConnections, NetId, PinConnection};
///
/// let mut conns = NetConnections::new();
/// conns.add(PinConnection::new("1", NetId(0)));  // Pin 1 -> VCC
/// conns.add(PinConnection::new("2", NetId(1)));  // Pin 2 -> GND
///
/// assert!(conns.contains_net(NetId(0)));
/// assert!(conns.pin_net("1") == Some(NetId(0)));
/// ```
#[derive(Component, Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct NetConnections {
    connections: Vec<PinConnection>,
}

impl NetConnections {
    /// Create an empty net connections collection.
    #[inline]
    pub fn new() -> Self {
        NetConnections {
            connections: Vec::new(),
        }
    }

    /// Create with pre-allocated capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        NetConnections {
            connections: Vec::with_capacity(capacity),
        }
    }

    /// Add a pin connection.
    #[inline]
    pub fn add(&mut self, connection: PinConnection) {
        self.connections.push(connection);
    }

    /// Check if any pin connects to the given net.
    pub fn contains_net(&self, net: NetId) -> bool {
        self.connections.iter().any(|c| c.net == net)
    }

    /// Get the net for a specific pin.
    pub fn pin_net(&self, pin: &str) -> Option<NetId> {
        self.connections.iter().find(|c| c.pin == pin).map(|c| c.net)
    }

    /// Get all pins connected to a specific net.
    pub fn pins_on_net(&self, net: NetId) -> impl Iterator<Item = &str> {
        self.connections
            .iter()
            .filter(move |c| c.net == net)
            .map(|c| c.pin.as_str())
    }

    /// Iterate over all connections.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &PinConnection> {
        self.connections.iter()
    }

    /// Number of connections.
    #[inline]
    pub fn len(&self) -> usize {
        self.connections.len()
    }

    /// Check if empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }
}

impl IntoIterator for NetConnections {
    type Item = PinConnection;
    type IntoIter = std::vec::IntoIter<PinConnection>;

    fn into_iter(self) -> Self::IntoIter {
        self.connections.into_iter()
    }
}

impl<'a> IntoIterator for &'a NetConnections {
    type Item = &'a PinConnection;
    type IntoIter = std::slice::Iter<'a, PinConnection>;

    fn into_iter(self) -> Self::IntoIter {
        self.connections.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refdes_prefix() {
        assert_eq!(RefDes::new("R1").prefix(), "R");
        assert_eq!(RefDes::new("LED2").prefix(), "LED");
        assert_eq!(RefDes::new("U100").prefix(), "U");
        assert_eq!(RefDes::new("C42").prefix(), "C");
    }

    #[test]
    fn test_refdes_number() {
        assert_eq!(RefDes::new("R1").number(), Some(1));
        assert_eq!(RefDes::new("LED2").number(), Some(2));
        assert_eq!(RefDes::new("U100").number(), Some(100));
        assert_eq!(RefDes::new("C42").number(), Some(42));
        assert_eq!(RefDes::new("TEST").number(), None);
    }

    #[test]
    fn test_net_connections() {
        let mut conns = NetConnections::new();
        let vcc = NetId(0);
        let gnd = NetId(1);

        conns.add(PinConnection::new("1", vcc));
        conns.add(PinConnection::new("2", gnd));

        assert!(conns.contains_net(vcc));
        assert!(conns.contains_net(gnd));
        assert!(!conns.contains_net(NetId(99)));

        assert_eq!(conns.pin_net("1"), Some(vcc));
        assert_eq!(conns.pin_net("2"), Some(gnd));
        assert_eq!(conns.pin_net("3"), None);

        let pins_on_vcc: Vec<_> = conns.pins_on_net(vcc).collect();
        assert_eq!(pins_on_vcc, vec!["1"]);
    }

    #[test]
    fn test_net_connections_iterator() {
        let mut conns = NetConnections::new();
        conns.add(PinConnection::new("1", NetId(0)));
        conns.add(PinConnection::new("2", NetId(1)));

        let count = conns.iter().count();
        assert_eq!(count, 2);
        assert_eq!(conns.len(), 2);
        assert!(!conns.is_empty());
    }
}
