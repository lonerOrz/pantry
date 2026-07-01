use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::PreviewPayload;

#[derive(Clone)]
pub struct MemoryCache {
    entries: Arc<Mutex<HashMap<PathBuf, PreviewPayload>>>,
    order: Arc<Mutex<VecDeque<PathBuf>>>,
    max_size: usize,
}

impl MemoryCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            order: Arc::new(Mutex::new(VecDeque::new())),
            max_size,
        }
    }

    pub fn get(&self, path: &Path) -> Option<PreviewPayload> {
        let entries = self.entries.lock().ok()?;
        let mut order = self.order.lock().ok()?;

        if let Some(payload) = entries.get(path) {
            if let Some(pos) = order.iter().position(|p| p == path) {
                order.remove(pos);
            }
            order.push_back(path.to_path_buf());
            return Some(payload.clone());
        }
        None
    }

    pub fn insert(&self, path: PathBuf, payload: PreviewPayload) {
        let mut entries = match self.entries.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        let mut order = match self.order.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        if entries.insert(path.clone(), payload).is_some() {
            if let Some(pos) = order.iter().position(|p| p == &path) {
                order.remove(pos);
            }
            order.push_back(path);
            return;
        }

        order.push_back(path.clone());

        if order.len() > self.max_size
            && let Some(oldest) = order.pop_front()
        {
            entries.remove(&oldest);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(s: &str) -> PreviewPayload {
        PreviewPayload::Text(s.to_string())
    }

    #[test]
    fn get_returns_inserted() {
        let cache = MemoryCache::new(4);
        cache.insert(PathBuf::from("a"), payload("A"));
        assert!(matches!(cache.get(Path::new("a")), Some(PreviewPayload::Text(ref s)) if s == "A"));
    }

    #[test]
    fn get_miss_returns_none() {
        let cache = MemoryCache::new(4);
        assert!(cache.get(Path::new("nope")).is_none());
    }

    #[test]
    fn lru_evicts_oldest() {
        let cache = MemoryCache::new(2);
        cache.insert(PathBuf::from("a"), payload("A"));
        cache.insert(PathBuf::from("b"), payload("B"));
        cache.insert(PathBuf::from("c"), payload("C")); // evicts "a"
        assert!(cache.get(Path::new("a")).is_none());
        assert!(cache.get(Path::new("b")).is_some());
        assert!(cache.get(Path::new("c")).is_some());
    }

    #[test]
    fn get_refreshes_recency() {
        let cache = MemoryCache::new(2);
        cache.insert(PathBuf::from("a"), payload("A"));
        cache.insert(PathBuf::from("b"), payload("B"));
        cache.get(Path::new("a")); // refresh "a" → now "b" is oldest
        cache.insert(PathBuf::from("c"), payload("C")); // evicts "b"
        assert!(cache.get(Path::new("a")).is_some());
        assert!(cache.get(Path::new("b")).is_none());
    }

    #[test]
    fn insert_overwrites_existing() {
        let cache = MemoryCache::new(2);
        cache.insert(PathBuf::from("a"), payload("A"));
        cache.insert(PathBuf::from("a"), payload("A2"));
        assert!(
            matches!(cache.get(Path::new("a")), Some(PreviewPayload::Text(ref s)) if s == "A2")
        );
    }
}
