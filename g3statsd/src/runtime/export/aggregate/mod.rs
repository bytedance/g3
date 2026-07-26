/*
 * SPDX-License-Identifier: Apache-2.0
 * SPDX-FileCopyrightText: 2025 ByteDance and/or its affiliates.
 * SPDX-FileCopyrightText: 2026 VEY-OSS Developers.
 */

use std::sync::Arc;
use std::time::Duration;

use ahash::AHashMap;
use chrono::{DateTime, Utc};
use tokio::sync::mpsc;

use g3_types::metrics::MetricTagMap;

use crate::types::{MetricName, MetricRecord, MetricType, MetricValue};

struct InnerMap<T> {
    inner: AHashMap<Arc<MetricTagMap>, T>,
}

impl<T> Default for InnerMap<T> {
    fn default() -> Self {
        InnerMap {
            inner: AHashMap::default(),
        }
    }
}

pub(crate) trait AggregateExport {
    fn emit_interval(&self) -> Duration;

    fn emit_gauge(
        &mut self,
        name: &MetricName,
        values: &AHashMap<Arc<MetricTagMap>, GaugeStoreValue>,
    );
    fn emit_counter(
        &mut self,
        name: &MetricName,
        values: &AHashMap<Arc<MetricTagMap>, CounterStoreValue>,
    );
}

pub(crate) struct AggregateExportRuntime<T: AggregateExport> {
    exporter: T,
    receiver: mpsc::UnboundedReceiver<(DateTime<Utc>, MetricRecord)>,
    store_time: DateTime<Utc>,

    counter: AHashMap<Arc<MetricName>, InnerMap<CounterStoreValue>>,
    gauge: AHashMap<Arc<MetricName>, InnerMap<GaugeStoreValue>>,
}

pub(crate) struct CounterStoreValue {
    pub(crate) time: DateTime<Utc>,
    pub(crate) sum: MetricValue,
    pub(crate) diff: MetricValue,
}

pub(crate) struct GaugeStoreValue {
    pub(crate) time: DateTime<Utc>,
    pub(crate) value: MetricValue,
}

impl<T: AggregateExport> AggregateExportRuntime<T> {
    pub(crate) fn new(
        exporter: T,
        receiver: mpsc::UnboundedReceiver<(DateTime<Utc>, MetricRecord)>,
    ) -> Self {
        AggregateExportRuntime {
            exporter,
            receiver,
            store_time: Utc::now(),
            counter: AHashMap::default(),
            gauge: AHashMap::default(),
        }
    }

    pub(crate) async fn into_running(mut self) {
        const BATCH_SIZE: usize = 128;

        let mut buf = Vec::with_capacity(BATCH_SIZE);

        let emit_interval = self.exporter.emit_interval();
        let mut emit_interval = tokio::time::interval(emit_interval);

        loop {
            buf.clear();

            tokio::select! {
                biased;

                _ = emit_interval.tick() => {
                    self.retain();
                    self.emit();
                }
                n = self.receiver.recv_many(&mut buf, BATCH_SIZE) => {
                    if n == 0 {
                        self.emit();
                        break;
                    }

                    for (_time, record) in buf.drain(..) {
                        self.add_record(record);
                    }
                }
            }
        }
    }

    fn retain(&mut self) {
        self.gauge.retain(|_, inner| {
            inner.inner.retain(|_, v| v.time >= self.store_time);
            !inner.inner.is_empty()
        });
        self.counter.retain(|_, inner| {
            inner.inner.retain(|_, v| v.time >= self.store_time);
            !inner.inner.is_empty()
        });
        self.store_time = Utc::now();
    }

    fn emit(&mut self) {
        for (name, inner) in &self.gauge {
            self.exporter.emit_gauge(name, &inner.inner);
        }
        for (name, inner) in &self.counter {
            self.exporter.emit_counter(name, &inner.inner);
        }
    }

    fn add_record(&mut self, record: MetricRecord) {
        match record.r#type {
            MetricType::Counter => {
                self.counter
                    .entry(record.name.clone())
                    .or_default()
                    .inner
                    .entry(record.tag_map.clone())
                    .and_modify(|v| {
                        if v.time < self.store_time {
                            v.time = self.store_time;
                            v.sum += record.value;
                            v.diff = record.value;
                        } else {
                            v.sum += record.value;
                            v.diff += record.value;
                        }
                    })
                    .or_insert(CounterStoreValue {
                        time: self.store_time,
                        sum: record.value,
                        diff: record.value,
                    });
            }
            MetricType::Gauge => {
                let inner = self.gauge.entry(record.name.clone()).or_default();
                inner.inner.insert(
                    record.tag_map,
                    GaugeStoreValue {
                        time: self.store_time,
                        value: record.value,
                    },
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use g3_types::metrics::MetricTagMap;

    struct TestExporter {
        counters: AHashMap<MetricName, AHashMap<Arc<MetricTagMap>, (MetricValue, MetricValue)>>,
    }

    impl AggregateExport for TestExporter {
        fn emit_interval(&self) -> Duration {
            Duration::from_secs(10)
        }

        fn emit_gauge(
            &mut self,
            _name: &MetricName,
            _values: &AHashMap<Arc<MetricTagMap>, GaugeStoreValue>,
        ) {
        }

        fn emit_counter(
            &mut self,
            name: &MetricName,
            values: &AHashMap<Arc<MetricTagMap>, CounterStoreValue>,
        ) {
            let map = self.counters.entry(name.clone()).or_default();
            for (tags, v) in values {
                map.insert(tags.clone(), (v.sum, v.diff));
            }
        }
    }

    #[test]
    fn test_counter_diff_accumulation_and_reset() {
        let (_tx, rx) = mpsc::unbounded_channel();
        let exporter = TestExporter {
            counters: AHashMap::default(),
        };
        let mut runtime = AggregateExportRuntime::new(exporter, rx);

        let name = Arc::new(MetricName::parse("test.counter").unwrap());
        let tag_map = Arc::new(MetricTagMap::default());

        // Interval 1 - First record
        runtime.add_record(MetricRecord {
            name: name.clone(),
            tag_map: tag_map.clone(),
            r#type: MetricType::Counter,
            value: MetricValue::Signed(10),
        });

        // Interval 1 - Second record (same interval)
        runtime.add_record(MetricRecord {
            name: name.clone(),
            tag_map: tag_map.clone(),
            r#type: MetricType::Counter,
            value: MetricValue::Signed(5),
        });

        // Verify state before retain
        let counter_entry = &runtime.counter.get(&name).unwrap().inner[&tag_map];
        assert_eq!(counter_entry.sum, MetricValue::Signed(15));
        assert_eq!(counter_entry.diff, MetricValue::Signed(15));

        // Simulate tick / interval transition
        runtime.retain();

        // Interval 2 - First record in new interval
        runtime.add_record(MetricRecord {
            name: name.clone(),
            tag_map: tag_map.clone(),
            r#type: MetricType::Counter,
            value: MetricValue::Signed(3),
        });

        // Verify diff reset for new interval, sum accumulated
        let counter_entry = &runtime.counter.get(&name).unwrap().inner[&tag_map];
        assert_eq!(counter_entry.sum, MetricValue::Signed(18));
        assert_eq!(counter_entry.diff, MetricValue::Signed(3));

        // Interval 2 - Second record in new interval
        runtime.add_record(MetricRecord {
            name: name.clone(),
            tag_map: tag_map.clone(),
            r#type: MetricType::Counter,
            value: MetricValue::Signed(7),
        });

        let counter_entry = &runtime.counter.get(&name).unwrap().inner[&tag_map];
        assert_eq!(counter_entry.sum, MetricValue::Signed(25));
        assert_eq!(counter_entry.diff, MetricValue::Signed(10));
    }
}
