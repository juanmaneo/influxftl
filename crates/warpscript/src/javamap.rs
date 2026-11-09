// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! A model of the iteration order of `java.util.HashMap` (and `HashSet`).
//!
//! Some Warp 10 results are Java hash maps whose iteration order leaks into WarpScript (the
//! maps a pickle decodes to, the maps a pickle is written from). [`JavaHashMap`] reproduces
//! that order: buckets indexed by the spread hash code, insertion order within a bucket,
//! the table doubling when the size exceeds three quarters of the capacity. Bins are never
//! turned into trees (Java does so past 8 colliding keys in a table of 64 buckets or more).

/// `HashMap.MAXIMUM_CAPACITY`.
const MAXIMUM_CAPACITY: usize = 1 << 30;

/// The entries of a Java `HashMap` in iteration order, given the Java `hashCode` of each key.
#[derive(Clone, Debug)]
pub struct JavaHashMap<K, V> {
    /// The buckets; empty until the first insertion (Java allocates lazily).
    table: Vec<Vec<(i32, K, V)>>,
    /// `threshold`: the size past which the table doubles (the initial capacity while the
    /// table is not allocated).
    threshold: usize,
    /// The number of entries.
    size: usize,
}

/// `HashMap.tableSizeFor`.
pub fn table_size_for(capacity: usize) -> usize {
    capacity.clamp(1, MAXIMUM_CAPACITY).next_power_of_two()
}

/// `HashMap.hash`: the spread hash code.
fn spread(hash: i32) -> usize {
    let h = hash as u32;
    (h ^ (h >> 16)) as usize
}

impl<K: PartialEq, V> Default for JavaHashMap<K, V> {
    fn default() -> Self {
        JavaHashMap::new()
    }
}

impl<K: PartialEq, V> JavaHashMap<K, V> {
    /// `new HashMap()`.
    pub fn new() -> Self {
        JavaHashMap {
            table: Vec::new(),
            threshold: 0,
            size: 0,
        }
    }

    /// `new HashMap(initialCapacity)`.
    pub fn with_capacity(capacity: usize) -> Self {
        JavaHashMap {
            table: Vec::new(),
            threshold: table_size_for(capacity),
            size: 0,
        }
    }

    /// `new HashSet(collection)`'s map: `new HashMap(max(size / .75f + 1, 16))`.
    pub fn for_set_copy(size: usize) -> Self {
        JavaHashMap::with_capacity((((size as f32) / 0.75f32) as usize + 1).max(16))
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.size
    }

    /// `true` when there is no entry.
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// `HashMap.resize`.
    fn resize(&mut self) {
        let old_cap = self.table.len();
        let old_thr = self.threshold;
        let mut new_thr = 0;
        // (Tables never reach `MAXIMUM_CAPACITY` buckets here: the input sizes are bounded.)
        let new_cap = if old_cap > 0 {
            if old_cap >= 16 {
                new_thr = old_thr * 2;
            }
            old_cap * 2
        } else if old_thr > 0 {
            old_thr
        } else {
            new_thr = 12;
            16
        };
        if new_thr == 0 {
            new_thr = ((new_cap as f32) * 0.75f32) as usize;
        }
        self.threshold = new_thr;
        let old = std::mem::take(&mut self.table);
        self.table = (0..new_cap).map(|_| Vec::new()).collect();
        // Java splits each bin in two, keeping the relative order: the same as re-inserting
        // the entries in iteration order.
        for (hash, key, value) in old.into_iter().flatten() {
            let index = spread(hash) & (new_cap - 1);
            self.table[index].push((hash, key, value));
        }
    }

    /// `HashMap.put`: insert or replace (a replaced entry keeps its position).
    pub fn put(&mut self, hash: i32, key: K, value: V) {
        if self.table.is_empty() {
            self.resize();
        }
        let index = spread(hash) & (self.table.len() - 1);
        let bin = &mut self.table[index];
        if let Some(entry) = bin.iter_mut().find(|(h, k, _)| *h == hash && *k == key) {
            entry.2 = value;
            return;
        }
        bin.push((hash, key, value));
        self.size += 1;
        if self.size > self.threshold {
            self.resize();
        }
    }

    /// `HashMap.putAll`: presize from the number of entries, then put them all.
    pub fn put_all(&mut self, entries: Vec<(i32, K, V)>) {
        let count = entries.len();
        if count > 0 {
            if self.table.is_empty() {
                let target = ((count as f32) / 0.75f32 + 1.0f32) as usize;
                // `if (t > threshold) threshold = tableSizeFor(t)`: the threshold of an empty
                // table is a power of two, so this is the larger of both.
                self.threshold = self.threshold.max(table_size_for(target));
            } else {
                while count > self.threshold {
                    self.resize();
                }
            }
        }
        for (hash, key, value) in entries {
            self.put(hash, key, value);
        }
    }

    /// The entries in iteration order.
    pub fn iter(&self) -> impl Iterator<Item = &(i32, K, V)> {
        self.table.iter().flatten()
    }

    /// The entries in iteration order, consumed.
    pub fn into_entries(self) -> Vec<(i32, K, V)> {
        self.table.into_iter().flatten().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(map: &JavaHashMap<i32, ()>) -> Vec<i32> {
        map.iter().map(|(_, k, ())| *k).collect()
    }

    #[test]
    fn orders_like_java() {
        let mut map = JavaHashMap::new();
        assert!(map.is_empty());
        for k in [17, 1, 33, 2] {
            map.put(k, k, ());
        }
        map.put(1, 1, ());
        assert_eq!(map.len(), 4);
        assert_eq!(keys(&map), vec![17, 1, 33, 2]);
        for k in 3..14 {
            map.put(k, k, ());
        }
        // 15 entries: the table doubled to 32 buckets.
        assert_eq!(keys(&map)[..4], [1, 33, 2, 3]);
        let mut small = JavaHashMap::with_capacity(0);
        for k in [3, 1, 2] {
            small.put(k, k, ());
        }
        assert_eq!(keys(&small), vec![1, 2, 3]);
        let mut all: JavaHashMap<i32, ()> = JavaHashMap::with_capacity(0);
        all.put_all((0..20).rev().map(|k| (k, k, ())).collect());
        assert_eq!(keys(&all), (0..20).collect::<Vec<_>>());
        all.put_all((20..100).map(|k| (k, k, ())).collect());
        all.put_all(Vec::new());
        assert_eq!(all.len(), 100);
        let set: JavaHashMap<i32, ()> = JavaHashMap::for_set_copy(30);
        assert_eq!(set.threshold, 64);
        assert_eq!(JavaHashMap::<i32, ()>::default().into_entries().len(), 0);
        assert_eq!(table_size_for(usize::MAX), MAXIMUM_CAPACITY);
    }
}
