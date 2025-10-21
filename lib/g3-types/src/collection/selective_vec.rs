/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::cmp::Ordering;
use std::hash::{BuildHasher, Hash, Hasher};
use std::str::FromStr;
use std::sync::atomic;

use foldhash::fast::FixedState;
use rand::seq::IndexedRandom;
use smallvec::SmallVec;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectivePickPolicy {
    Random,
    Serial,
    RoundRobin,
    Ketama,
    Rendezvous,
    JumpHash,
}

impl FromStr for SelectivePickPolicy {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "random" => Ok(SelectivePickPolicy::Random),
            "serial" | "sequence" => Ok(SelectivePickPolicy::Serial),
            "roundrobin" | "rr" | "round_robin" => Ok(SelectivePickPolicy::RoundRobin),
            "ketama" => Ok(SelectivePickPolicy::Ketama),
            "rendezvous" => Ok(SelectivePickPolicy::Rendezvous),
            "jump" | "jumphash" | "jump_hash" => Ok(SelectivePickPolicy::JumpHash),
            _ => Err(()),
        }
    }
}

pub trait SelectiveItem {
    fn weight(&self) -> f64;
    fn weight_u32(&self) -> u32 {
        // return the smallest integer greater than or equal to `self`
        self.weight().ceil() as u32
    }
    fn selective_hash<H: Hasher>(&self, state: &mut H);
}

impl<T: Hash> SelectiveItem for T {
    fn weight(&self) -> f64 {
        1.0
    }

    fn weight_u32(&self) -> u32 {
        1
    }

    fn selective_hash<H: Hasher>(&self, state: &mut H) {
        self.hash(state);
    }
}

pub struct SelectiveVecBuilder<T> {
    inner: Vec<T>,
}

impl<T: SelectiveItem> SelectiveVecBuilder<T> {
    pub fn new() -> Self {
        SelectiveVecBuilder { inner: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        SelectiveVecBuilder {
            inner: Vec::with_capacity(capacity),
        }
    }

    pub fn with_inner(inner: Vec<T>) -> Self {
        SelectiveVecBuilder { inner }
    }

    pub fn insert(&mut self, value: T) {
        self.inner.push(value);
    }

    pub fn build(self) -> Option<SelectiveVec<T>> {
        if self.inner.is_empty() {
            return None;
        }

        let mut weighted = false;
        let weight = self.inner[0].weight();
        for item in &self.inner {
            if item.weight().ne(&weight) {
                weighted = true;
                break;
            }
        }

        let mut nodes = self.inner;
        // reserve order for equal nodes
        nodes.sort_by(|a, b| {
            b.weight()
                .partial_cmp(&a.weight())
                .unwrap_or(Ordering::Equal)
        });

        let ketama_ring = ketama_ring_create(&nodes);

        Some(SelectiveVec {
            weighted,
            inner: nodes,
            rr_id: atomic::AtomicUsize::new(0),
            ketama_ring,
        })
    }
}

fn ketama_ring_create<T: SelectiveItem>(nodes: &[T]) -> Vec<(usize, u32)> {
    // This constant is copied from nginx. It will create 160 points per weight unit. For
    // example, a weight of 2 will create 320 points on the ring.
    const POINT_MULTIPLE: u32 = 160;
    let total_weights: u32 = nodes.iter().map(|v| v.weight_u32()).sum();
    let mut ring = Vec::with_capacity((total_weights * POINT_MULTIPLE) as usize);

    for (i, node) in nodes.iter().enumerate() {
        let mut hasher = crc32fast::Hasher::new();
        node.selective_hash(&mut hasher);

        let num_points = node.weight_u32() * POINT_MULTIPLE;

        let mut prev_hash: u32 = 0;
        for _ in 0..num_points {
            let mut hasher = hasher.clone();
            hasher.update(&prev_hash.to_le_bytes());

            let hash = hasher.finalize();
            ring.push((i, hash));
            prev_hash = hash;
        }
    }

    // Sort and remove any duplicates.
    ring.sort_unstable_by(|v1, v2| v1.1.cmp(&v2.1));
    ring.dedup_by(|v1, v2| v1.1 == v2.1);

    ring
}

impl<T: SelectiveItem> Default for SelectiveVecBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SelectiveVec<T: SelectiveItem> {
    weighted: bool,
    inner: Vec<T>,
    rr_id: atomic::AtomicUsize,
    ketama_ring: Vec<(usize, u32)>,
}

macro_rules! panic_on_empty {
    () => {{ panic!("do panic check before pick node") }};
}

impl<T: SelectiveItem> SelectiveVec<T> {
    #[cfg(feature = "resolve")]
    pub(crate) fn new_basic(inner: Vec<T>) -> Self {
        debug_assert!(!inner.is_empty());
        SelectiveVec {
            weighted: false,
            inner,
            rr_id: Default::default(),
            ketama_ring: Vec::new(),
        }
    }

    pub fn pick_random(&self) -> &T {
        match self.inner.len() {
            0 => panic_on_empty!(),
            1 => &self.inner[0],
            _ => {
                let mut rng = rand::rng();
                if self.weighted {
                    self.inner
                        .choose_weighted(&mut rng, |v| v.weight())
                        .unwrap_or(&self.inner[0])
                } else {
                    self.inner.choose(&mut rng).unwrap_or(&self.inner[0])
                }
            }
        }
    }

    pub fn pick_random_n(&self, n: usize) -> Vec<&T> {
        match self.inner.len() {
            0 => panic_on_empty!(),
            1 => vec![&self.inner[0]],
            _ => {
                let len = self.inner.len().min(n);

                let mut rng = rand::rng();
                if self.weighted {
                    self.inner
                        .choose_multiple_weighted(&mut rng, len, |v| v.weight())
                        .unwrap_or_else(|_| self.inner.choose_multiple(&mut rng, len))
                        .collect()
                } else {
                    self.inner.choose_multiple(&mut rng, len).collect()
                }
            }
        }
    }

    pub fn pick_serial(&self) -> &T {
        if self.inner.is_empty() {
            panic_on_empty!()
        } else {
            &self.inner[0]
        }
    }

    pub fn pick_serial_n(&self, n: usize) -> Vec<&T> {
        if self.inner.is_empty() {
            panic_on_empty!()
        } else {
            let mut len = self.inner.len();
            if len > n {
                len = n;
            }

            let mut r = Vec::with_capacity(len);
            for item in &self.inner.as_slice()[0..len] {
                r.push(item);
            }
            r
        }
    }

    pub fn pick_round_robin(&self) -> &T {
        match self.inner.len() {
            0 => panic_on_empty!(),
            1 => &self.inner[0],
            _ => {
                let mut id = self.rr_id.load(atomic::Ordering::Acquire);
                loop {
                    let mut next = id + 1;
                    if next >= self.inner.len() {
                        next = 0;
                    }

                    match self.rr_id.compare_exchange(
                        id,
                        next,
                        atomic::Ordering::AcqRel,
                        atomic::Ordering::Acquire,
                    ) {
                        Ok(_) => return self.inner.get(id).unwrap_or(&self.inner[0]),
                        Err(n) => id = n,
                    }
                }
            }
        }
    }

    pub fn pick_round_robin_n(&self, n: usize) -> Vec<&T> {
        match self.inner.len() {
            0 => panic_on_empty!(),
            1 => vec![&self.inner[0]],
            len => {
                let n = n.min(len);
                let mut id = self.rr_id.load(atomic::Ordering::Acquire);
                loop {
                    let mut end = id + n;
                    if end >= len {
                        end %= len;
                    }

                    match self.rr_id.compare_exchange(
                        id,
                        end,
                        atomic::Ordering::AcqRel,
                        atomic::Ordering::Acquire,
                    ) {
                        Ok(_) => {
                            let mut r = Vec::with_capacity(n);
                            if end <= id {
                                for item in &self.inner.as_slice()[id..] {
                                    r.push(item);
                                }
                                for item in &self.inner.as_slice()[0..end] {
                                    r.push(item);
                                }
                            } else {
                                for item in &self.inner.as_slice()[id..end] {
                                    r.push(item);
                                }
                            }
                            return r;
                        }
                        Err(n) => id = n,
                    }
                }
            }
        }
    }

    /// It outputs a bucket number in the range [0, slot_count)
    fn jump_hash<K>(key: &K, slot_count: u32) -> u32
    where
        K: Hash + ?Sized,
    {
        let mut h = FixedState::default().hash_one(key);
        let (mut b, mut j) = (-1i64, 0i64);
        while j < slot_count as i64 {
            b = j;
            h = h.wrapping_mul(2862933555777941757).wrapping_add(1);
            j = ((b.wrapping_add(1) as f64) * (((1u64 << 31) as f64) / (((h >> 33) + 1) as f64)))
                as i64;
        }
        b as u32
    }

    pub fn pick_jump<K>(&self, key: &K) -> &T
    where
        K: Hash + ?Sized,
    {
        match self.inner.len() {
            0 => panic_on_empty!(),
            1 => &self.inner[0],
            slot_count => {
                // no weight support
                let slot = Self::jump_hash(key, slot_count as u32);
                self.inner.get(slot as usize).unwrap_or(&self.inner[0])
            }
        }
    }

    fn rendezvous_hash<K>(item: &T, key: &K) -> u64
    where
        K: Hash + ?Sized,
    {
        let mut hasher = FixedState::default().build_hasher();
        key.hash(&mut hasher);
        item.selective_hash(&mut hasher);
        hasher.finish()
    }

    fn rendezvous_weighted_hash<K>(item: &T, key: &K) -> f64
    where
        K: Hash + ?Sized,
    {
        let mut hasher = FixedState::default().build_hasher();
        key.hash(&mut hasher);
        item.selective_hash(&mut hasher);
        let hash = hasher.finish() as f64;
        let distance = (hash / u64::MAX as f64).ln();
        distance / item.weight()
    }

    pub fn pick_rendezvous<K>(&self, key: &K) -> &T
    where
        K: Hash + ?Sized,
    {
        match self.inner.len() {
            0 => panic_on_empty!(),
            1 => &self.inner[0],
            _ => {
                let mut node = &self.inner[0];
                if self.weighted {
                    let mut final_value = 0f64;
                    for item in &self.inner {
                        let value = Self::rendezvous_weighted_hash(item, key);
                        if final_value < value {
                            final_value = value;
                            node = item;
                        }
                    }
                } else {
                    let mut final_value = 0u64;
                    for item in &self.inner {
                        let value = Self::rendezvous_hash(item, key);
                        if final_value < value {
                            final_value = value;
                            node = item;
                        }
                    }
                }
                node
            }
        }
    }

    pub fn pick_rendezvous_n<K>(&self, key: &K, n: usize) -> Vec<&T>
    where
        K: Hash + ?Sized,
    {
        match self.inner.len() {
            0 => panic_on_empty!(),
            1 => vec![&self.inner[0]],
            _ => {
                // use stack storage if less than or equal to 32 nodes
                let mut nodes = SmallVec::<[(&T, f64); 32]>::with_capacity(self.inner.len());
                if self.weighted {
                    for item in &self.inner {
                        let value = Self::rendezvous_weighted_hash(item, key);
                        nodes.push((item, value));
                    }
                } else {
                    for item in &self.inner {
                        let value = Self::rendezvous_hash(item, key) as f64;
                        nodes.push((item, value));
                    }
                }

                nodes.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
                if n < nodes.len() {
                    nodes.truncate(n);
                }
                nodes.into_iter().map(|n| n.0).collect()
            }
        }
    }

    fn ketama_ring_idx<K>(&self, key: &K) -> usize
    where
        K: Hash + ?Sized,
    {
        let mut hasher = crc32fast::Hasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finalize();

        match self.ketama_ring.binary_search_by(|v| v.1.cmp(&hash)) {
            Ok(i) => i, // found
            Err(i) => {
                // will be inserted here
                if i >= self.ketama_ring.len() {
                    // make sure we always get a valid node
                    0
                } else {
                    i
                }
            }
        }
    }

    pub fn pick_ketama<K>(&self, key: &K) -> &T
    where
        K: Hash + ?Sized,
    {
        match self.inner.len() {
            0 => panic_on_empty!(),
            1 => &self.inner[0],
            _ => {
                let idx = self.ketama_ring_idx(key);
                let node = &self.ketama_ring[idx];
                &self.inner[node.0]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct Node {
        name: String,
        weight: f64,
    }

    impl SelectiveItem for Node {
        fn weight(&self) -> f64 {
            self.weight
        }

        fn selective_hash<H: Hasher>(&self, state: &mut H) {
            self.name.hash(state);
        }
    }

    impl PartialEq for Node {
        fn eq(&self, other: &Self) -> bool {
            self.name.eq(&other.name)
        }
    }

    #[test]
    fn pick_one_from_one() {
        let node = Node {
            name: "test".to_string(),
            weight: 1f64,
        };

        let mut builder = SelectiveVecBuilder::with_capacity(1);
        builder.insert(node.clone());
        let vec = builder.build().unwrap();

        assert!(node.eq(vec.pick_serial()));
        assert!(node.eq(vec.pick_round_robin()));
        assert!(node.eq(vec.pick_random()));
        assert!(node.eq(vec.pick_rendezvous("k")));
        assert!(node.eq(vec.pick_jump("k")));
        assert!(node.eq(vec.pick_ketama("k")));
    }

    #[test]
    fn pick_one_from_two() {
        let node1 = Node {
            name: "node1".to_string(),
            weight: 1f64,
        };
        let node2 = Node {
            name: "node2".to_string(),
            weight: 1f64,
        };

        let mut builder = SelectiveVecBuilder::with_capacity(1);
        builder.insert(node1.clone());
        builder.insert(node2.clone());
        let vec = builder.build().unwrap();

        assert!(node1.eq(vec.pick_serial()));
        assert!(node1.eq(vec.pick_round_robin()));
        assert!(node2.eq(vec.pick_round_robin()));

        /*
        let mut see1 = false;
        let mut see2 = false;
        for _ in 0..100 {
            let node = vec.pick_random();
            if node.eq(&node1) {
                see1 = true;
            }
            if node.eq(&node2) {
                see2 = true;
            }
        }
        assert!(see1);
        assert!(see2);
         */

        let prev = vec.pick_rendezvous("k");
        let next = vec.pick_rendezvous("k");
        assert!(prev.eq(next));

        let prev = vec.pick_jump("k");
        let next = vec.pick_jump("k");
        assert!(prev.eq(next));

        let prev = vec.pick_ketama("k");
        let next = vec.pick_ketama("k");
        assert!(prev.eq(next));
    }

    #[test]
    fn pick_one_from_weighted_two() {
        let node1 = Node {
            name: "node1".to_string(),
            weight: 1f64,
        };
        let node2 = Node {
            name: "node2".to_string(),
            weight: 2f64,
        };

        let mut builder = SelectiveVecBuilder::with_capacity(1);
        builder.insert(node1.clone());
        builder.insert(node2.clone());
        let vec = builder.build().unwrap();

        assert!(node2.eq(vec.pick_serial()));
        assert!(node2.eq(vec.pick_round_robin()));

        /*
        let mut see1 = 0usize;
        let mut see2 = 0usize;
        for _ in 0..100 {
            let node = vec.pick_random();
            if node.eq(&node1) {
                see1 += 1;
            }
            if node.eq(&node2) {
                see2 += 1;
            }
        }
        assert!(see2 > see1);
         */

        let prev = vec.pick_rendezvous("k");
        let next = vec.pick_rendezvous("k");
        assert!(prev.eq(next));

        let prev = vec.pick_jump("k");
        let next = vec.pick_jump("k");
        assert!(prev.eq(next));

        let prev = vec.pick_ketama("k");
        let next = vec.pick_ketama("k");
        assert!(prev.eq(next));
    }

    #[test]
    fn pick_two_from_two() {
        let node1 = Node {
            name: "node1".to_string(),
            weight: 1f64,
        };
        let node2 = Node {
            name: "node2".to_string(),
            weight: 1f64,
        };

        let mut builder = SelectiveVecBuilder::with_capacity(1);
        builder.insert(node1.clone());
        builder.insert(node2.clone());
        let vec = builder.build().unwrap();

        let r = vec.pick_serial_n(2);
        assert_eq!(r.len(), 2);
        assert!(r[0].eq(&node1));
        assert!(r[1].eq(&node2));

        let r = vec.pick_round_robin_n(2);
        assert_eq!(r.len(), 2);
        assert!(r[0].eq(&node1));
        assert!(r[1].eq(&node2));

        /*
        let mut see1 = false;
        let mut see2 = false;
        let r = vec.pick_random_n(2);
        assert_eq!(r.len(), 2);
        assert!(r[0].ne(r[1]));
        for item in &r {
            if node1.eq(item) {
                see1 = true;
            }
            if node2.eq(item) {
                see2 = true;
            }
        }
        assert!(see1);
        assert!(see2);
         */

        let r1 = vec.pick_rendezvous_n("k", 2);
        let r2 = vec.pick_rendezvous_n("k", 2);
        assert_eq!(r1.len(), 2);
        assert_eq!(r2.len(), 2);
        assert!(r1[0].ne(r1[1]));
        assert!(r1[0].eq(r2[0]));
        assert!(r1[1].eq(r2[1]));
    }

    #[test]
    fn pick_two_from_weighted_two() {
        let node1 = Node {
            name: "node1".to_string(),
            weight: 1f64,
        };
        let node2 = Node {
            name: "node2".to_string(),
            weight: 2f64,
        };

        let mut builder = SelectiveVecBuilder::with_capacity(1);
        builder.insert(node1.clone());
        builder.insert(node2.clone());
        let vec = builder.build().unwrap();

        let r = vec.pick_serial_n(2);
        assert_eq!(r.len(), 2);
        assert!(r[0].eq(&node2));
        assert!(r[1].eq(&node1));

        let r = vec.pick_round_robin_n(2);
        assert_eq!(r.len(), 2);
        assert!(r[0].eq(&node2));
        assert!(r[1].eq(&node1));

        /*
        let mut see1 = false;
        let mut see2 = false;
        let r = vec.pick_random_n(2);
        assert_eq!(r.len(), 2);
        assert!(r[0].ne(r[1]));
        for item in &r {
            if node1.eq(item) {
                see1 = true;
            }
            if node2.eq(item) {
                see2 = true;
            }
        }
        assert!(see1);
        assert!(see2);
         */

        let r1 = vec.pick_rendezvous_n("k", 2);
        let r2 = vec.pick_rendezvous_n("k", 2);
        assert_eq!(r1.len(), 2);
        assert_eq!(r2.len(), 2);
        assert!(r1[0].ne(r1[1]));
        assert!(r1[0].eq(r2[0]));
        assert!(r1[1].eq(r2[1]));
    }

    #[test]
    fn pick_two_from_three() {
        let node1 = Node {
            name: "node1".to_string(),
            weight: 1f64,
        };
        let node2 = Node {
            name: "node2".to_string(),
            weight: 1f64,
        };
        let node3 = Node {
            name: "node3".to_string(),
            weight: 1f64,
        };

        let mut builder = SelectiveVecBuilder::with_capacity(1);
        builder.insert(node1.clone());
        builder.insert(node2.clone());
        builder.insert(node3.clone());
        let vec = builder.build().unwrap();

        let r = vec.pick_serial_n(2);
        assert_eq!(r.len(), 2);
        assert!(r[0].eq(&node1));
        assert!(r[1].eq(&node2));

        let r = vec.pick_round_robin_n(2);
        assert_eq!(r.len(), 2);
        assert!(r[0].eq(&node1));
        assert!(r[1].eq(&node2));

        let r = vec.pick_round_robin_n(2);
        assert_eq!(r.len(), 2);
        assert!(r[0].eq(&node3));
        assert!(r[1].eq(&node1));

        /*
        let mut see1 = false;
        let mut see2 = false;
        for _ in 0..100 {
            let r = vec.pick_random_n(2);
            assert_eq!(r.len(), 2);
            assert!(r[0].ne(r[1]));
            for item in &r {
                if node1.eq(item) {
                    see1 = true;
                }
                if node2.eq(item) {
                    see2 = true;
                }
            }
        }
        assert!(see1);
        assert!(see2);
         */

        let r1 = vec.pick_rendezvous_n("k", 2);
        let r2 = vec.pick_rendezvous_n("k", 2);
        assert_eq!(r1.len(), 2);
        assert_eq!(r2.len(), 2);
        assert!(r1[0].ne(r1[1]));
        assert!(r1[0].eq(r2[0]));
        assert!(r1[1].eq(r2[1]));
    }

    #[test]
    fn pick_two_from_weighted_three() {
        let node1 = Node {
            name: "node1".to_string(),
            weight: 1f64,
        };
        let node2 = Node {
            name: "node2".to_string(),
            weight: 2f64,
        };
        let node3 = Node {
            name: "node3".to_string(),
            weight: 3f64,
        };

        let mut builder = SelectiveVecBuilder::with_capacity(1);
        builder.insert(node1.clone());
        builder.insert(node2.clone());
        builder.insert(node3.clone());
        let vec = builder.build().unwrap();

        let r = vec.pick_serial_n(2);
        assert_eq!(r.len(), 2);
        assert!(r[0].eq(&node3));
        assert!(r[1].eq(&node2));

        let r = vec.pick_round_robin_n(2);
        assert_eq!(r.len(), 2);
        assert!(r[0].eq(&node3));
        assert!(r[1].eq(&node2));

        let r = vec.pick_round_robin_n(2);
        assert_eq!(r.len(), 2);
        assert!(r[0].eq(&node1));
        assert!(r[1].eq(&node3));

        /*
        let mut see1 = 0usize;
        let mut see2 = 0usize;
        let mut see3 = 0usize;
        for _ in 0..100 {
            let r = vec.pick_random_n(2);
            assert_eq!(r.len(), 2);
            assert!(r[0].ne(r[1]));
            for item in r {
                if node1.eq(item) {
                    see1 += 1;
                }
                if node2.eq(item) {
                    see2 += 1;
                }
                if node3.eq(item) {
                    see3 += 1;
                }
            }
        }
        assert!(see3 > see2);
        assert!(see2 > see1);
         */

        let r1 = vec.pick_rendezvous_n("k", 2);
        let r2 = vec.pick_rendezvous_n("k", 2);
        assert_eq!(r1.len(), 2);
        assert_eq!(r2.len(), 2);
        assert!(r1[0].ne(r1[1]));
        assert!(r1[0].eq(r2[0]));
        assert!(r1[1].eq(r2[1]));
    }
}
