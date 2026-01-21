//! Net registry for interning net names.
//!
//! Net names are stored once and referenced by integer IDs for efficient
//! comparison and storage. This is essential for performance when dealing
//! with thousands of components and their net connections.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::components::NetId;

/// Registry for interning net names to integer IDs.
///
/// Net names are strings like "VCC", "GND", "CLK", etc. Instead of storing
/// and comparing these strings everywhere, we intern them to `NetId(u32)` values.
///
/// This provides:
/// - O(1) net comparison (integer comparison vs string comparison)
/// - Reduced memory usage (single copy of each net name)
/// - Fast hashing for net-based lookups
///
/// # Examples
///
/// ```
/// use cypcb_world::NetRegistry;
///
/// let mut registry = NetRegistry::new();
///
/// // Intern net names
/// let vcc = registry.intern("VCC");
/// let gnd = registry.intern("GND");
///
/// // Same name returns same ID
/// let vcc2 = registry.intern("VCC");
/// assert_eq!(vcc, vcc2);
///
/// // Different names return different IDs
/// assert_ne!(vcc, gnd);
///
/// // Look up names by ID
/// assert_eq!(registry.name(vcc), Some("VCC"));
/// assert_eq!(registry.name(gnd), Some("GND"));
/// ```
#[derive(Resource, Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetRegistry {
    /// Stored net names, indexed by NetId value.
    names: Vec<String>,
    /// Lookup from name to NetId for fast interning.
    #[serde(skip)]
    lookup: HashMap<String, NetId>,
}

impl NetRegistry {
    /// Create a new empty net registry.
    #[inline]
    pub fn new() -> Self {
        NetRegistry {
            names: Vec::new(),
            lookup: HashMap::new(),
        }
    }

    /// Create a registry with pre-allocated capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        NetRegistry {
            names: Vec::with_capacity(capacity),
            lookup: HashMap::with_capacity(capacity),
        }
    }

    /// Intern a net name, returning its unique ID.
    ///
    /// If the name has been interned before, returns the existing ID.
    /// Otherwise, creates a new ID and stores the name.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_world::NetRegistry;
    ///
    /// let mut registry = NetRegistry::new();
    /// let id1 = registry.intern("NET1");
    /// let id2 = registry.intern("NET1");
    /// assert_eq!(id1, id2);
    /// ```
    pub fn intern(&mut self, name: &str) -> NetId {
        if let Some(&id) = self.lookup.get(name) {
            return id;
        }

        let id = NetId::new(self.names.len() as u32);
        self.names.push(name.to_string());
        self.lookup.insert(name.to_string(), id);
        id
    }

    /// Get the name for a NetId.
    ///
    /// Returns `None` if the ID is not valid (out of range).
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_world::{NetRegistry, NetId};
    ///
    /// let mut registry = NetRegistry::new();
    /// let vcc = registry.intern("VCC");
    ///
    /// assert_eq!(registry.name(vcc), Some("VCC"));
    /// assert_eq!(registry.name(NetId::new(999)), None);
    /// ```
    #[inline]
    pub fn name(&self, id: NetId) -> Option<&str> {
        self.names.get(id.0 as usize).map(String::as_str)
    }

    /// Look up a NetId by name without interning.
    ///
    /// Returns `None` if the name has not been interned.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_world::NetRegistry;
    ///
    /// let mut registry = NetRegistry::new();
    /// registry.intern("VCC");
    ///
    /// assert!(registry.get("VCC").is_some());
    /// assert!(registry.get("UNKNOWN").is_none());
    /// ```
    #[inline]
    pub fn get(&self, name: &str) -> Option<NetId> {
        self.lookup.get(name).copied()
    }

    /// Check if a name has been interned.
    #[inline]
    pub fn contains(&self, name: &str) -> bool {
        self.lookup.contains_key(name)
    }

    /// Iterate over all interned nets.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_world::NetRegistry;
    ///
    /// let mut registry = NetRegistry::new();
    /// registry.intern("VCC");
    /// registry.intern("GND");
    ///
    /// let nets: Vec<_> = registry.iter().collect();
    /// assert_eq!(nets.len(), 2);
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = (NetId, &str)> + '_ {
        self.names
            .iter()
            .enumerate()
            .map(|(i, s)| (NetId::new(i as u32), s.as_str()))
    }

    /// Get the number of interned nets.
    #[inline]
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Check if the registry is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// Clear all interned nets.
    pub fn clear(&mut self) {
        self.names.clear();
        self.lookup.clear();
    }

    /// Rebuild the lookup table from names.
    ///
    /// Call this after deserialization to restore the lookup HashMap.
    pub fn rebuild_lookup(&mut self) {
        self.lookup.clear();
        for (i, name) in self.names.iter().enumerate() {
            self.lookup.insert(name.clone(), NetId::new(i as u32));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intern_returns_same_id() {
        let mut registry = NetRegistry::new();
        let id1 = registry.intern("VCC");
        let id2 = registry.intern("VCC");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_intern_different_names() {
        let mut registry = NetRegistry::new();
        let vcc = registry.intern("VCC");
        let gnd = registry.intern("GND");
        assert_ne!(vcc, gnd);
    }

    #[test]
    fn test_name_lookup() {
        let mut registry = NetRegistry::new();
        let vcc = registry.intern("VCC");
        let gnd = registry.intern("GND");

        assert_eq!(registry.name(vcc), Some("VCC"));
        assert_eq!(registry.name(gnd), Some("GND"));
        assert_eq!(registry.name(NetId::new(999)), None);
    }

    #[test]
    fn test_get_lookup() {
        let mut registry = NetRegistry::new();
        registry.intern("VCC");

        assert!(registry.get("VCC").is_some());
        assert!(registry.get("UNKNOWN").is_none());
    }

    #[test]
    fn test_iter() {
        let mut registry = NetRegistry::new();
        registry.intern("VCC");
        registry.intern("GND");
        registry.intern("CLK");

        let nets: Vec<_> = registry.iter().collect();
        assert_eq!(nets.len(), 3);
        assert_eq!(nets[0].1, "VCC");
        assert_eq!(nets[1].1, "GND");
        assert_eq!(nets[2].1, "CLK");
    }

    #[test]
    fn test_len_and_is_empty() {
        let mut registry = NetRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        registry.intern("VCC");
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_clear() {
        let mut registry = NetRegistry::new();
        registry.intern("VCC");
        registry.intern("GND");

        registry.clear();
        assert!(registry.is_empty());
        assert!(registry.get("VCC").is_none());
    }

    #[test]
    fn test_rebuild_lookup() {
        let mut registry = NetRegistry::new();
        registry.intern("VCC");
        registry.intern("GND");

        // Simulate what happens after deserialization
        registry.lookup.clear();
        assert!(registry.get("VCC").is_none());

        // Rebuild restores lookup
        registry.rebuild_lookup();
        assert_eq!(registry.get("VCC"), Some(NetId::new(0)));
        assert_eq!(registry.get("GND"), Some(NetId::new(1)));
    }

    #[test]
    fn test_ids_are_sequential() {
        let mut registry = NetRegistry::new();
        let id0 = registry.intern("NET0");
        let id1 = registry.intern("NET1");
        let id2 = registry.intern("NET2");

        assert_eq!(id0.0, 0);
        assert_eq!(id1.0, 1);
        assert_eq!(id2.0, 2);
    }
}
