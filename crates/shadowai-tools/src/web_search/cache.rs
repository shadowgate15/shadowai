/// In-memory cache for web search results.
///
/// Keyed on the normalized query string; each entry stores the cached response body (JSON)
/// plus its insertion timestamp. Default TTL is 120 seconds. The cache evicts oldest entries
/// once it exceeds 10,000 items to bound memory usage.
use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct CachedEntry {
    cached_response: String,
    inserted_at: Instant,
}

/// Shared cache state protected by a Mutex. Initialized once at startup via OnceLock.
static CACHE: OnceLock<Arc<Mutex<HashMap<String, CachedEntry>>>> = OnceLock::new();

fn get_cache() -> Arc<Mutex<HashMap<String, CachedEntry>>> {
    CACHE
        .get_or_init(|| Arc::new(Mutex::new(HashMap::<String, CachedEntry>::default())))
        .clone()
}

/// Check whether a cached entry exists and is still within TTL (default 120s).
///
/// Returns `Some(response)` if the query was served from cache, `None` otherwise.
pub fn check(query: &str) -> Option<String> {
    let now = Instant::now();

    // Lock once; borrow the HashMap for the duration of this function.
    match get_cache().lock() {
        Ok(map) => {
            if let Some(entry) = map.get(query) {
                if now.duration_since(entry.inserted_at) < Duration::from_secs(120) {
                    return Some(entry.cached_response.clone());
                }
            }
            None
        }
        Err(_) => {
            // Cache poisoned — panic is acceptable for an in-process cache.
            panic!("Cache Mutex was poisoned");
        }
    }
}

/// Store a response in the cache keyed by `query`. Evicts expired entries and clears
/// if capacity exceeds 10,000.
pub fn store(query: &str, response: String) {
    let now = Instant::now();

    // Lock once; borrow the HashMap for the duration of this function.
    match get_cache().lock() {
        Ok(mut map) => {
            map.retain(|_, entry| now.duration_since(entry.inserted_at) < Duration::from_secs(120));

            map.insert(
                query.to_string(),
                CachedEntry {
                    cached_response: response,
                    inserted_at: now,
                },
            );

            // If cache exceeds 10_000 items, clear everything (simple eviction for MVP).
            if map.len() > 10_000 {
                map.clear();
            }
        }
        Err(_) => {
            panic!("Cache Mutex was poisoned");
        }
    }
}
