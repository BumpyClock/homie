use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const CACHE_MAX_ENTRIES: usize = 100;

#[derive(Clone)]
pub(super) struct CacheEntry {
    pub(super) value: serde_json::Value,
    expires_at: Instant,
}

static FETCH_CACHE: OnceLock<Mutex<HashMap<String, CacheEntry>>> = OnceLock::new();
static SEARCH_CACHE: OnceLock<Mutex<HashMap<String, CacheEntry>>> = OnceLock::new();

pub(super) fn normalize_cache_key(value: &str) -> String {
    value.trim().to_lowercase()
}

pub(super) fn read_cache(
    cache: &Mutex<HashMap<String, CacheEntry>>,
    key: &str,
) -> Option<CacheEntry> {
    let mut guard = cache.lock().ok()?;
    if let Some(entry) = guard.get(key) {
        if entry.expires_at > Instant::now() {
            return Some(entry.clone());
        }
    }
    guard.remove(key);
    None
}

pub(super) fn write_cache(
    cache: &Mutex<HashMap<String, CacheEntry>>,
    key: &str,
    value: serde_json::Value,
    ttl_ms: u64,
) {
    if ttl_ms == 0 {
        return;
    }
    let mut guard = match cache.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    if guard.len() >= CACHE_MAX_ENTRIES {
        if let Some(oldest) = guard.keys().next().cloned() {
            guard.remove(&oldest);
        }
    }
    guard.insert(
        key.to_string(),
        CacheEntry {
            value,
            expires_at: Instant::now() + Duration::from_millis(ttl_ms),
        },
    );
}

pub(super) fn fetch_cache() -> &'static Mutex<HashMap<String, CacheEntry>> {
    FETCH_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) fn search_cache() -> &'static Mutex<HashMap<String, CacheEntry>> {
    SEARCH_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}
