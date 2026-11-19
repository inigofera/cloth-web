//! Minimal per-key fixed-window rate limiter.
//!
//! Single-process home server, one known user: a bounded in-memory counter is
//! the right tool. A key that exceeds `limit` requests inside its `window` is
//! refused until the window rolls; the counter resets with it. Keys are
//! per-user identifiers, so the map cannot grow unboundedly.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct RateLimiter {
    state: Mutex<HashMap<String, (Instant, u32)>>,
}

impl RateLimiter {
    pub fn new() -> RateLimiter {
        RateLimiter {
            state: Mutex::new(HashMap::new()),
        }
    }

    /// Returns true when `key` may perform another action in this window and
    /// records it; false when the window's budget is exhausted.
    pub fn allow(&self, key: &str, limit: u32, window: Duration) -> bool {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Drop entries whose window has fully rolled: at most one scan, and the
        // map holds one entry per user, so this is cheap.
        let now = Instant::now();
        state.retain(|_, (start, count)| now.duration_since(*start) < window && *count > 0);

        match state.get_mut(key) {
            Some((start, count)) if now.duration_since(*start) < window => {
                if *count >= limit {
                    false
                } else {
                    *count += 1;
                    true
                }
            }
            _ => {
                state.insert(key.to_string(), (now, 1));
                true
            }
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_up_to_the_limit() {
        let rl = RateLimiter::new();
        for _ in 0..3 {
            assert!(rl.allow("u1", 3, Duration::from_secs(60)));
        }
        assert!(!rl.allow("u1", 3, Duration::from_secs(60)));
    }

    #[test]
    fn keys_are_independent() {
        let rl = RateLimiter::new();
        assert!(rl.allow("u1", 1, Duration::from_secs(60)));
        assert!(!rl.allow("u1", 1, Duration::from_secs(60)));
        // Exhausting u1's budget must not affect u2.
        assert!(rl.allow("u2", 1, Duration::from_secs(60)));
    }

    #[test]
    fn window_reset_allows_again() {
        let rl = RateLimiter::new();
        assert!(rl.allow("u1", 1, Duration::from_millis(5)));
        assert!(!rl.allow("u1", 1, Duration::from_millis(5)));
        std::thread::sleep(Duration::from_millis(20));
        assert!(rl.allow("u1", 1, Duration::from_millis(5)));
    }
}
