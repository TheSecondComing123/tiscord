use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct CachedImage {
    pub protocol_data: Vec<u8>,
    pub width: u16,
    pub height: u16,
}

pub struct ImageCache {
    cache: HashMap<String, CachedImage>,
    capacity: usize,
    order: Vec<String>, // LRU tracking
}

impl ImageCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: HashMap::new(),
            capacity,
            order: Vec::new(),
        }
    }

    pub fn get(&mut self, url: &str) -> Option<&CachedImage> {
        if self.cache.contains_key(url) {
            // Move to end (most recently used)
            self.order.retain(|u| u != url);
            self.order.push(url.to_string());
            self.cache.get(url)
        } else {
            None
        }
    }

    pub fn insert(&mut self, url: String, image: CachedImage) {
        if self.cache.len() >= self.capacity && !self.cache.contains_key(&url) {
            // Evict oldest
            if let Some(oldest) = self.order.first().cloned() {
                self.cache.remove(&oldest);
                self.order.remove(0);
            }
        }
        self.order.retain(|u| u != &url);
        self.order.push(url.clone());
        self.cache.insert(url, image);
    }

    pub fn contains(&self, url: &str) -> bool {
        self.cache.contains_key(url)
    }
}

impl Default for ImageCache {
    fn default() -> Self {
        Self::new(50)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_image(tag: u8) -> CachedImage {
        CachedImage {
            protocol_data: vec![tag],
            width: 10,
            height: 10,
        }
    }

    #[test]
    fn test_insert_and_get() {
        let mut cache = ImageCache::new(10);
        cache.insert("https://example.com/a.png".to_string(), make_image(1));
        assert!(cache.contains("https://example.com/a.png"));
        let img = cache.get("https://example.com/a.png");
        assert!(img.is_some());
        assert_eq!(img.unwrap().protocol_data, vec![1]);
    }

    #[test]
    fn test_get_missing_returns_none() {
        let mut cache = ImageCache::new(10);
        assert!(cache.get("https://example.com/missing.png").is_none());
    }

    #[test]
    fn test_eviction_removes_oldest() {
        let mut cache = ImageCache::new(2);
        cache.insert("https://example.com/a.png".to_string(), make_image(1));
        cache.insert("https://example.com/b.png".to_string(), make_image(2));
        // Inserting a third entry should evict the oldest ("a")
        cache.insert("https://example.com/c.png".to_string(), make_image(3));
        assert!(!cache.contains("https://example.com/a.png"), "oldest should be evicted");
        assert!(cache.contains("https://example.com/b.png"));
        assert!(cache.contains("https://example.com/c.png"));
    }

    #[test]
    fn test_access_updates_lru_order() {
        let mut cache = ImageCache::new(2);
        cache.insert("https://example.com/a.png".to_string(), make_image(1));
        cache.insert("https://example.com/b.png".to_string(), make_image(2));
        // Access "a" to make it most recently used
        let _ = cache.get("https://example.com/a.png");
        // Now insert "c" — "b" should be evicted as the new oldest
        cache.insert("https://example.com/c.png".to_string(), make_image(3));
        assert!(cache.contains("https://example.com/a.png"), "recently accessed entry should survive");
        assert!(!cache.contains("https://example.com/b.png"), "least recently used should be evicted");
        assert!(cache.contains("https://example.com/c.png"));
    }

    #[test]
    fn test_reinserting_existing_key_does_not_evict() {
        let mut cache = ImageCache::new(2);
        cache.insert("https://example.com/a.png".to_string(), make_image(1));
        cache.insert("https://example.com/b.png".to_string(), make_image(2));
        // Re-insert an existing key — should not evict anything
        cache.insert("https://example.com/a.png".to_string(), make_image(99));
        assert!(cache.contains("https://example.com/a.png"));
        assert!(cache.contains("https://example.com/b.png"));
        // Value should be updated
        let img = cache.get("https://example.com/a.png").unwrap();
        assert_eq!(img.protocol_data, vec![99]);
    }
}
