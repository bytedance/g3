/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::sync::atomic;

use metrohash::MetroHash64;
use rand::seq::SliceRandom;
use smallvec::SmallVec;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectivePickPolicy {
    Random,
    Serial,
    RoundRobin,
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
            "rendezvous" => Ok(SelectivePickPolicy::Rendezvous),
            "jump" | "jumphash" | "jump_hash" => Ok(SelectivePickPolicy::JumpHash),
            _ => Err(()),
        }
    }
}

pub trait SelectiveItem {
    fn weight(&self) -> f64;
}

pub trait SelectiveHash {
    fn selective_hash<H: Hasher>(&self, state: &mut H);
}

pub struct SelectiveVecBuilder<T> {
    inner: Vec<T>,
}

#[derive(Debug, Error)]
pub enum SelectiveVecBuildError {
    #[error("no node has been added")]
    Empty,
    #[error("some nodes is not sortable")]
    NotSortable,
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

    pub fn insert(&mut self, value: T) {
        self.inner.push(value);
    }

    pub fn build(self) -> Result<SelectiveVec<T>, SelectiveVecBuildError> {
        if self.inner.is_empty() {
            return Err(SelectiveVecBuildError::Empty);
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
        let mut sort_ok = true;
        // reserve order for equal nodes
        nodes.sort_by(|a, b| {
            b.weight().partial_cmp(&a.weight()).unwrap_or_else(|| {
                sort_ok = false;
                Ordering::Equal
            })
        });
        if !sort_ok {
            return Err(SelectiveVecBuildError::NotSortable);
        }

        Ok(SelectiveVec {
            weighted,
            inner: nodes,
            rr_id: atomic::AtomicUsize::new(0),
        })
    }
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
}

impl<T: SelectiveItem> SelectiveVec<T> {
    pub fn empty() -> Self {
        SelectiveVec {
            weighted: false,
            inner: Vec::new(),
            rr_id: atomic::AtomicUsize::new(0),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

macro_rules! panic_on_empty {
    () => {{
        panic!("do panic check before pick node")
    }};
}

impl<T: SelectiveItem> SelectiveVec<T> {
    pub fn pick_random(&self) -> &T {
        match self.inner.len() {
            0 => panic_on_empty!(),
            1 => &self.inner[0],
            _ => {
                let mut rng = rand::thread_rng();
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

                let mut rng = rand::thread_rng();
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
            _ => {
                let len = self.inner.len();
                let n = n.min(len);
                let mut id = self.rr_id.load(atomic::Ordering::Acquire);
                loop {
                    let mut next = id + n;
                    if next >= len {
                        next -= len;
                    }

                    match self.rr_id.compare_exchange(
                        id,
                        next,
                        atomic::Ordering::AcqRel,
                        atomic::Ordering::Acquire,
                    ) {
                        Ok(_) => {
                            return if next <= id {
                                let mut r = Vec::with_capacity(n);
                                for item in &self.inner.as_slice()[id..] {
                                    r.push(item);
                                }
                                for item in &self.inner.as_slice()[0..next] {
                                    r.push(item);
                                }
                                r
                            } else {
                                let mut r = Vec::with_capacity(n);
                                for item in &self.inner.as_slice()[id..next] {
                                    r.push(item);
                                }
                                r
                            }
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
        let mut hasher = MetroHash64::new();
        key.hash(&mut hasher);
        let mut h = hasher.finish();
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
}

impl<T> SelectiveVec<T>
where
    T: SelectiveItem + SelectiveHash,
{
    fn rendezvous_hash<K>(item: &T, key: &K) -> u64
    where
        K: Hash + ?Sized,
    {
        let mut hasher = MetroHash64::new();
        key.hash(&mut hasher);
        item.selective_hash(&mut hasher);
        hasher.finish()
    }

    fn rendezvous_weighted_hash<K>(item: &T, key: &K) -> f64
    where
        K: Hash + ?Sized,
    {
        let mut hasher = MetroHash64::new();
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
    }

    impl SelectiveHash for Node {
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
    }

    #[test]
    #[ignore]
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

        let prev = vec.pick_rendezvous("k");
        let next = vec.pick_rendezvous("k");
        assert!(prev.eq(next));

        let prev = vec.pick_jump("k");
        let next = vec.pick_jump("k");
        assert!(prev.eq(next));
    }

    #[test]
    #[ignore]
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

        let prev = vec.pick_rendezvous("k");
        let next = vec.pick_rendezvous("k");
        assert!(prev.eq(next));

        let prev = vec.pick_jump("k");
        let next = vec.pick_jump("k");
        assert!(prev.eq(next));
    }

    #[test]
    #[ignore]
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

        let r1 = vec.pick_rendezvous_n("k", 2);
        let r2 = vec.pick_rendezvous_n("k", 2);
        assert_eq!(r1.len(), 2);
        assert_eq!(r2.len(), 2);
        assert!(r1[0].ne(r1[1]));
        assert!(r1[0].eq(r2[0]));
        assert!(r2[1].eq(r2[1]));
    }

    #[test]
    #[ignore]
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

        let r1 = vec.pick_rendezvous_n("k", 2);
        let r2 = vec.pick_rendezvous_n("k", 2);
        assert_eq!(r1.len(), 2);
        assert_eq!(r2.len(), 2);
        assert!(r1[0].ne(r1[1]));
        assert!(r1[0].eq(r2[0]));
        assert!(r2[1].eq(r2[1]));
    }

    #[test]
    #[ignore]
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

        let r1 = vec.pick_rendezvous_n("k", 2);
        let r2 = vec.pick_rendezvous_n("k", 2);
        assert_eq!(r1.len(), 2);
        assert_eq!(r2.len(), 2);
        assert!(r1[0].ne(r1[1]));
        assert!(r1[0].eq(r2[0]));
        assert!(r2[1].eq(r2[1]));
    }

    #[test]
    #[ignore]
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

        let r1 = vec.pick_rendezvous_n("k", 2);
        let r2 = vec.pick_rendezvous_n("k", 2);
        assert_eq!(r1.len(), 2);
        assert_eq!(r2.len(), 2);
        assert!(r1[0].ne(r1[1]));
        assert!(r1[0].eq(r2[0]));
        assert!(r2[1].eq(r2[1]));
    }
}
