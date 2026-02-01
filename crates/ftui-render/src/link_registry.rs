#![forbid(unsafe_code)]

//! OSC 8 hyperlink registry.
//!
//! The `LinkRegistry` maps link IDs to URLs. This allows cells to store
//! compact 24-bit link IDs instead of full URL strings.
//!
//! # Usage
//!
//! ```
//! use ftui_render::link_registry::LinkRegistry;
//!
//! let mut registry = LinkRegistry::new();
//! let id = registry.register("https://example.com");
//! assert_eq!(registry.get(id), Some("https://example.com"));
//! ```

use std::collections::HashMap;

/// Registry for OSC 8 hyperlink URLs.
#[derive(Debug, Clone, Default)]
pub struct LinkRegistry {
    /// Map from link ID to URL.
    links: HashMap<u32, String>,
    /// Next available link ID.
    next_id: u32,
}

impl LinkRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            links: HashMap::new(),
            next_id: 1, // Reserve 0 for "no link"
        }
    }

    /// Register a URL and return its link ID.
    ///
    /// If the URL is already registered, returns the existing ID.
    pub fn register(&mut self, url: &str) -> u32 {
        // Check if URL already exists
        for (&id, existing) in &self.links {
            if existing == url {
                return id;
            }
        }

        // Allocate new ID
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.links.insert(id, url.to_string());
        id
    }

    /// Get the URL for a link ID.
    pub fn get(&self, id: u32) -> Option<&str> {
        self.links.get(&id).map(|s| s.as_str())
    }

    /// Remove a link by ID.
    pub fn remove(&mut self, id: u32) -> Option<String> {
        self.links.remove(&id)
    }

    /// Clear all links.
    pub fn clear(&mut self) {
        self.links.clear();
        self.next_id = 1;
    }

    /// Number of registered links.
    pub fn len(&self) -> usize {
        self.links.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_get() {
        let mut registry = LinkRegistry::new();
        let id = registry.register("https://example.com");
        assert_eq!(registry.get(id), Some("https://example.com"));
    }

    #[test]
    fn deduplication() {
        let mut registry = LinkRegistry::new();
        let id1 = registry.register("https://example.com");
        let id2 = registry.register("https://example.com");
        assert_eq!(id1, id2);
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn multiple_urls() {
        let mut registry = LinkRegistry::new();
        let id1 = registry.register("https://one.com");
        let id2 = registry.register("https://two.com");
        assert_ne!(id1, id2);
        assert_eq!(registry.get(id1), Some("https://one.com"));
        assert_eq!(registry.get(id2), Some("https://two.com"));
    }

    #[test]
    fn remove() {
        let mut registry = LinkRegistry::new();
        let id = registry.register("https://example.com");
        assert!(registry.get(id).is_some());
        registry.remove(id);
        assert!(registry.get(id).is_none());
    }

    #[test]
    fn clear() {
        let mut registry = LinkRegistry::new();
        registry.register("https://one.com");
        registry.register("https://two.com");
        assert_eq!(registry.len(), 2);
        registry.clear();
        assert!(registry.is_empty());
    }
}
