//! Bounded in-memory cache for the `/files/*path` endpoint.
//!
//! Serving a thumbnail used to mean re-fetching the full-resolution original
//! from Supabase Storage and re-decoding it on the request thread — once per
//! `<img>` on the page, and once again on every reload. Uploads now also write
//! small thumbnail objects next to the image (see `upload.rs`), so most
//! responses are a copy of an already-stored object; this cache removes the
//! remaining upstream round-trips for repeat requests, and keeps resized
//! originals (`?width=` larger than any generated thumbnail) from being
//! recomputed on every request.
//!
//! Keys encode the exact response a URL asks for — the object path plus the
//! effective `?width=` — so `/files/a.jpg?w=480` can never answer a request
//! for the untouched original. Eviction is least-recently-used across both an
//! entry count and a byte budget, so browsing a big wardrobe for a long
//! session cannot grow the map without limit.

use std::collections::HashMap;
use std::sync::Mutex;

/// A finished, ready-to-send image response.
#[derive(Clone)]
pub struct CachedImage {
    pub body: Vec<u8>,
    pub content_type: String,
}

pub struct ImageCache {
    /// Hard cap on the number of cached responses.
    pub capacity: usize,
    /// Hard cap on total cached bytes; a response larger than this is simply
    /// not stored, so one huge image can never evict everything else.
    pub max_bytes: usize,
    state: Mutex<CacheState>,
}

struct CacheState {
    entries: HashMap<String, CacheSlot>,
    /// Monotonic clock that ranks entries by recency.
    tick: u64,
    stored_bytes: usize,
}

struct CacheSlot {
    body: Vec<u8>,
    content_type: String,
    used: u64,
    size: usize,
}

impl ImageCache {
    pub fn new(capacity: usize, max_bytes: usize) -> ImageCache {
        ImageCache {
            capacity: if capacity > 0 { capacity } else { 1 },
            max_bytes: if max_bytes > 0 { max_bytes } else { 1 },
            state: Mutex::new(CacheState {
                entries: HashMap::new(),
                tick: 0,
                stored_bytes: 0,
            }),
        }
    }

    /// Return the cached response for `key`, marking it as most recently used.
    pub fn get(&self, key: &str) -> Option<CachedImage> {
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let tick = state.tick + 1;
        state.tick = tick;

        match state.entries.get_mut(key) {
            None => None,
            Some(slot) => {
                slot.used = tick;
                Some(CachedImage {
                    body: slot.body.clone(),
                    content_type: slot.content_type.clone(),
                })
            },
        }
    }

    /// Store a response, then drop least-recently-used entries until both the
    /// entry count and the byte budget fit again.
    pub fn put(&self, key: &str, image: CachedImage) {
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let size = image.body.len();

        // Bigger than the whole budget: keeping it would mean dropping it
        // immediately, so just skip caching.
        if size > self.max_bytes {
            return;
        }

        // Retire any previous occupant of this key before accounting for the
        // new one, so `stored_bytes` cannot drift when a URL is served twice.
        let mut displaced = 0;
        match state.entries.remove(key) {
            None => {},
            Some(old) => { displaced = old.size; },
        };
        state.stored_bytes = state.stored_bytes - displaced;

        let tick = state.tick + 1;
        state.tick = tick;
        state.entries.insert(key.to_string(), CacheSlot {
            body: image.body,
            content_type: image.content_type,
            used: tick,
            size: size,
        });
        state.stored_bytes = state.stored_bytes + size;

        while state.entries.len() > self.capacity || state.stored_bytes > self.max_bytes {
            // Oldest last-use wins. The map holds at most `capacity + 1`
            // entries here, so the scan costs one short pass per insert.
            let mut victim_key: Option<String> = None;
            let mut oldest_used = u64::MAX;
            for (candidate_key, slot) in state.entries.iter() {
                if slot.used < oldest_used {
                    oldest_used = slot.used;
                    victim_key = Some(candidate_key.to_string());
                }
            }

            match victim_key {
                None => break,
                Some(victim) => {
                    let mut evicted = 0;
                    match state.entries.remove(&victim) {
                        None => {},
                        Some(old) => { evicted = old.size; },
                    };
                    state.stored_bytes = state.stored_bytes - evicted;
                },
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hits_and_misses_are_keyed_exactly() {
        let cache = ImageCache::new(4, 1024);
        cache.put("users/u/a.jpg?width=480", CachedImage {
            body: vec![1u8, 2u8],
            content_type: "image/jpeg".to_string(),
        });

        let hit = cache.get("users/u/a.jpg?width=480");
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().body.len(), 2);

        // A different `?width=` on the same object is a different response, and
        // the untouched original is a third: none of them may answer for another.
        assert!(cache.get("users/u/a.jpg?width=96").is_none());
        assert!(cache.get("users/u/a.jpg?original").is_none());
    }

    #[test]
    fn entries_never_outlive_the_budgets() {
        let cache = ImageCache::new(2, 100);
        for i in 0..10 {
            cache.put(
                &format!("users/u/item{i}.jpg?width=480"),
                CachedImage {
                    body: vec![7u8; 40],
                    content_type: "image/jpeg".to_string(),
                },
            );
        }

        let state = cache.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        assert!(state.entries.len() <= 2);
        // The byte budget is the binding one here: two 40-byte slots fit, three
        // would not.
        assert!(state.stored_bytes <= 100);
    }

    #[test]
    fn oversize_responses_are_skipped_not_stored() {
        let cache = ImageCache::new(4, 100);
        cache.put("users/u/huge.jpg", CachedImage {
            body: vec![0u8; 200],
            content_type: "image/jpeg".to_string(),
        });
        // Storing it would have evicted everything else, so it is not cached at
        // all ...
        assert!(cache.get("users/u/huge.jpg").is_none());

        // ... and the responses around it stay available.
        cache.put("users/u/small.jpg?width=480", CachedImage {
            body: vec![0u8; 10],
            content_type: "image/jpeg".to_string(),
        });
        assert!(cache.get("users/u/small.jpg?width=480").is_some());
    }

    #[test]
    fn eviction_retires_the_least_recently_used() {
        let cache = ImageCache::new(2, 4096);
        cache.put("a", CachedImage { body: vec![0u8; 8], content_type: "image/jpeg".to_string() });
        cache.put("b", CachedImage { body: vec![0u8; 8], content_type: "image/jpeg".to_string() });
        // Reading `a` makes `b` the older of the two.
        cache.get("a");
        cache.put("c", CachedImage { body: vec![0u8; 8], content_type: "image/jpeg".to_string() });

        assert!(cache.get("b").is_none());
        assert!(cache.get("a").is_some());
        assert!(cache.get("c").is_some());
    }
}
