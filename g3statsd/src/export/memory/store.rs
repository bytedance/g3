/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use ahash::AHashMap;
use chrono::{DateTime, Utc};

use g3_types::metrics::MetricTagMap;

use crate::runtime::export::{CounterStoreValue, GaugeStoreValue};
use crate::types::{MetricName, MetricRecord, MetricType, MetricValue};

struct InnerMap<T> {
    inner: AHashMap<Arc<MetricTagMap>, VecDeque<T>>,
}

impl<T> Default for InnerMap<T> {
    fn default() -> Self {
        InnerMap {
            inner: AHashMap::default(),
        }
    }
}

impl InnerMap<CounterStoreValue> {
    fn add(
        &mut self,
        time: DateTime<Utc>,
        store_count: usize,
        tag_map: Arc<MetricTagMap>,
        value: MetricValue,
    ) {
        let mut store_v = CounterStoreValue {
            time,
            sum: value,
            diff: value,
        };
        let queue = self
            .inner
            .entry(tag_map)
            .or_insert_with(|| VecDeque::with_capacity(store_count));
        if let Some(last_v) = queue.front() {
            store_v.sum += last_v.sum;
        }
        queue.push_front(store_v);
        queue.truncate(store_count);
    }
}

impl InnerMap<GaugeStoreValue> {
    fn add(
        &mut self,
        time: DateTime<Utc>,
        store_count: usize,
        tag_map: Arc<MetricTagMap>,
        value: MetricValue,
    ) {
        let store_v = GaugeStoreValue { time, value };
        let queue = self.inner.entry(tag_map).or_default();
        queue.push_front(store_v);
        queue.truncate(store_count);
    }
}

type CounterInnerMap = Arc<Mutex<InnerMap<CounterStoreValue>>>;
type GaugeInnerMap = Arc<Mutex<InnerMap<GaugeStoreValue>>>;

pub(super) struct MemoryStore {
    counter: Mutex<AHashMap<Arc<MetricName>, CounterInnerMap>>,
    gauge: Mutex<AHashMap<Arc<MetricName>, GaugeInnerMap>>,
}

impl Default for MemoryStore {
    fn default() -> Self {
        MemoryStore {
            counter: Mutex::new(AHashMap::default()),
            gauge: Mutex::new(AHashMap::default()),
        }
    }
}

impl MemoryStore {
    pub(super) fn add_record(
        &self,
        time: DateTime<Utc>,
        store_count: usize,
        record: &MetricRecord,
    ) {
        match record.r#type {
            MetricType::Counter => {
                let mut map = self.counter.lock().unwrap();
                let slot = map.entry(record.name.clone()).or_default().clone();
                drop(map);

                let mut inner = slot.lock().unwrap();
                inner.add(time, store_count, record.tag_map.clone(), record.value);
            }
            MetricType::Gauge => {
                let mut map = self.gauge.lock().unwrap();
                let slot = map.entry(record.name.clone()).or_default().clone();
                drop(map);

                let mut inner = slot.lock().unwrap();
                inner.add(time, store_count, record.tag_map.clone(), record.value);
            }
        };
    }
}
