//! A bounded map for what the service keeps in memory: a cap on entries and one on bytes, the least
//! recently used entry going first past either, and a sweep that lets go of entries left unused
//! for a while (`Service`'s sweep runs it every few minutes).

use std::{
    borrow::Borrow,
    collections::HashMap,
    hash::Hash,
    time::{Duration, Instant},
};

/// What one cache may hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub entries: usize,
    pub bytes: usize,
    /// An entry not used for this long goes at the next sweep.
    pub idle: Duration,
}

/// A map within [`Limits`]. An entry larger than the byte cap on its own is not kept at all.
pub struct Lru<K, V> {
    limits: Limits,
    entries: HashMap<K, Slot<V>>,
    bytes: usize,
    /// Counts every use, so the least recently used entry is the one with the lowest count even
    /// where two were used at the same instant.
    clock: u64,
}

struct Slot<V> {
    value: V,
    bytes: usize,
    order: u64,
    used: Instant,
}

impl<K: Eq + Hash + Clone, V> Lru<K, V> {
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            entries: HashMap::new(),
            bytes: 0,
            clock: 0,
        }
    }

    pub fn limits(&self) -> Limits {
        self.limits
    }

    /// The value under `key`, marked as used at `now`.
    pub fn get<Q>(&mut self, key: &Q, now: Instant) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.clock += 1;
        let slot = self.entries.get_mut(key)?;
        slot.order = self.clock;
        slot.used = now;
        Some(&slot.value)
    }

    /// Keeps `value`, `bytes` large, as used at `now`, replacing what `key` held, and lets the least
    /// recently used entries go while either cap is passed. Returns whether it was kept.
    pub fn insert(&mut self, key: K, value: V, bytes: usize, now: Instant) -> bool {
        self.remove(&key);
        if bytes > self.limits.bytes || self.limits.entries == 0 {
            return false;
        }
        while self.entries.len() >= self.limits.entries || self.bytes + bytes > self.limits.bytes {
            if !self.evict() {
                break;
            }
        }
        self.clock += 1;
        self.bytes += bytes;
        self.entries.insert(
            key,
            Slot {
                value,
                bytes,
                order: self.clock,
                used: now,
            },
        );
        true
    }

    /// Lets every entry go that was last used [`Limits::idle`] or longer before `now`. Returns how
    /// many went.
    pub fn sweep(&mut self, now: Instant) -> usize {
        let idle = self.limits.idle;
        let before = self.entries.len();
        let mut freed = 0;
        self.entries.retain(|_, slot| {
            let keep = now.saturating_duration_since(slot.used) < idle;
            if !keep {
                freed += slot.bytes;
            }
            keep
        });
        self.bytes -= freed;
        before - self.entries.len()
    }

    /// Lets everything go. Returns how many entries and bytes went.
    pub fn clear(&mut self) -> (usize, usize) {
        let went = (self.entries.len(), self.bytes);
        self.entries.clear();
        self.bytes = 0;
        went
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }

    fn remove(&mut self, key: &K) {
        if let Some(slot) = self.entries.remove(key) {
            self.bytes -= slot.bytes;
        }
    }

    /// Lets the least recently used entry go; `false` when there was none.
    fn evict(&mut self) -> bool {
        let Some(oldest) = self
            .entries
            .iter()
            .min_by_key(|(_, slot)| slot.order)
            .map(|(key, _)| key.clone())
        else {
            return false;
        };
        self.remove(&oldest);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: Duration = Duration::from_secs(60);

    fn cache(entries: usize, bytes: usize) -> Lru<&'static str, u32> {
        Lru::new(Limits {
            entries,
            bytes,
            idle: 30 * MINUTE,
        })
    }

    #[test]
    fn past_the_entry_cap_the_least_recently_used_goes_first() {
        let now = Instant::now();
        let mut lru = cache(2, 1000);
        lru.insert("a", 1, 10, now);
        lru.insert("b", 2, 10, now);
        // Using `a` makes `b` the one that has waited longest.
        assert_eq!(lru.get(&"a", now), Some(&1));
        assert!(lru.insert("c", 3, 10, now));
        assert_eq!((lru.len(), lru.bytes()), (2, 20));
        assert!(lru.get(&"b", now).is_none());
        assert!(lru.get(&"a", now).is_some() && lru.get(&"c", now).is_some());
    }

    #[test]
    fn past_the_byte_cap_as_many_go_as_make_room_and_an_entry_too_big_is_not_kept() {
        let now = Instant::now();
        let mut lru = cache(100, 100);
        for (key, value) in [("a", 1), ("b", 2), ("c", 3), ("d", 4)] {
            lru.insert(key, value, 25, now);
        }
        assert_eq!(lru.bytes(), 100);
        // Sixty bytes need the room of the three oldest.
        assert!(lru.insert("big", 5, 60, now));
        assert_eq!((lru.len(), lru.bytes()), (2, 85));
        assert!(lru.get(&"d", now).is_some() && lru.get(&"big", now).is_some());
        // Larger than the whole cap: refused, and nothing else is let go for it.
        assert!(!lru.insert("huge", 6, 101, now));
        assert_eq!((lru.len(), lru.bytes()), (2, 85));
    }

    #[test]
    fn a_key_kept_again_replaces_its_value_and_its_size() {
        let now = Instant::now();
        let mut lru = cache(10, 100);
        lru.insert("a", 1, 40, now);
        lru.insert("a", 2, 10, now);
        assert_eq!((lru.len(), lru.bytes()), (1, 10));
        assert_eq!(lru.get(&"a", now), Some(&2));
    }

    #[test]
    fn a_sweep_lets_go_of_what_was_not_used_for_the_idle_time() {
        let start = Instant::now();
        let mut lru = cache(10, 100);
        lru.insert("old", 1, 30, start);
        lru.insert("used", 2, 20, start);
        lru.insert("new", 3, 10, start + 20 * MINUTE);
        // Using an entry restarts its time.
        lru.get(&"used", start + 25 * MINUTE);
        assert_eq!(lru.sweep(start + 29 * MINUTE), 0);
        assert_eq!(
            lru.sweep(start + 30 * MINUTE),
            1,
            "only `old` waited half an hour"
        );
        assert_eq!((lru.len(), lru.bytes()), (2, 30));
        assert_eq!(lru.sweep(start + 60 * MINUTE), 2);
        assert!(lru.is_empty() && lru.bytes() == 0);
    }

    #[test]
    fn clearing_says_what_went() {
        let now = Instant::now();
        let mut lru = cache(10, 100);
        lru.insert("a", 1, 30, now);
        lru.insert("b", 2, 20, now);
        assert_eq!(lru.clear(), (2, 50));
        assert_eq!((lru.len(), lru.bytes()), (0, 0));
        assert!(lru.insert("c", 3, 100, now), "the whole cap is free again");
    }
}
