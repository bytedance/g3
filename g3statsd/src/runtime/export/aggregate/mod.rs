/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use std::sync::Arc;
use std::time::Duration;

use ahash::AHashMap;
use anyhow::anyhow;
use chrono::{DateTime, TimeDelta, Utc};
use tokio::sync::mpsc;

use crate::types::{MetricName, MetricRecord, MetricTagMap, MetricType, MetricValue};

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
    fn expire_timeout(&self) -> Duration;

    async fn emit_gauge(
        &mut self,
        name: &MetricName,
        values: &AHashMap<Arc<MetricTagMap>, GaugeStoreValue>,
    );
    async fn emit_counter(
        &mut self,
        name: &MetricName,
        values: &AHashMap<Arc<MetricTagMap>, CounterStoreValue>,
    );
}

pub(crate) struct AggregateExportRuntime<T: AggregateExport> {
    exporter: T,
    receiver: mpsc::Receiver<(DateTime<Utc>, MetricRecord)>,
    expire_timeout: TimeDelta,

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
        receiver: mpsc::Receiver<(DateTime<Utc>, MetricRecord)>,
    ) -> anyhow::Result<Self> {
        let expire_timeout = TimeDelta::from_std(exporter.expire_timeout())
            .map_err(|e| anyhow!("invalid expire timeout value: {e}"))?;

        Ok(AggregateExportRuntime {
            exporter,
            receiver,
            expire_timeout,
            counter: AHashMap::default(),
            gauge: AHashMap::default(),
        })
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
                    self.emit().await;
                }
                n = self.receiver.recv_many(&mut buf, BATCH_SIZE) => {
                    if n == 0 {
                        self.emit().await;
                        break;
                    }

                    while let Some((time, record)) = buf.pop() {
                        self.add_record(time, record);
                    }
                }
            }
        }
    }

    fn retain(&mut self) {
        let now = Utc::now();
        let expire = now
            .checked_sub_signed(self.expire_timeout)
            .unwrap_or(DateTime::from_timestamp_nanos(0));

        self.gauge.retain(|_, inner| {
            inner.inner.retain(|_, v| v.time > expire);
            !inner.inner.is_empty()
        });
        self.counter.retain(|_, inner| {
            inner.inner.retain(|_, v| v.time > expire);
            !inner.inner.is_empty()
        });
    }

    async fn emit(&mut self) {
        for (name, inner) in &self.gauge {
            self.exporter.emit_gauge(name, &inner.inner).await;
        }
        for (name, inner) in &self.counter {
            self.exporter.emit_counter(name, &inner.inner).await;
        }
    }

    fn add_record(&mut self, time: DateTime<Utc>, record: MetricRecord) {
        match record.r#type {
            MetricType::Counter => {
                self.counter
                    .entry(record.name.clone())
                    .or_default()
                    .inner
                    .entry(record.tag_map.clone())
                    .and_modify(|v| {
                        v.time = time;
                        v.sum += record.value;
                        v.diff = record.value;
                    })
                    .or_insert(CounterStoreValue {
                        time,
                        sum: record.value,
                        diff: record.value,
                    });
            }
            MetricType::Gauge => {
                let inner = self.gauge.entry(record.name.clone()).or_default();
                inner.inner.insert(
                    record.tag_map,
                    GaugeStoreValue {
                        time,
                        value: record.value,
                    },
                );
            }
        }
    }
}
