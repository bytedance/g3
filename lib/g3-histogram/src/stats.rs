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

use std::sync::atomic::{AtomicU64, Ordering};

use hdrhistogram::{Counter, Histogram};
use portable_atomic::AtomicF64;

use super::Quantile;

pub struct HistogramQuantileStats {
    quantile: Quantile,
    value: AtomicU64,
}

impl HistogramQuantileStats {
    fn new(quantile: Quantile) -> Self {
        HistogramQuantileStats {
            quantile,
            value: AtomicU64::new(0),
        }
    }
}

pub struct HistogramStats {
    min: AtomicU64,
    max: AtomicU64,
    mean: AtomicF64,
    quantile: Vec<HistogramQuantileStats>,
}

impl HistogramStats {
    pub fn new() -> Self {
        HistogramStats {
            min: AtomicU64::new(0),
            max: AtomicU64::new(0),
            mean: AtomicF64::new(0.0_f64),
            quantile: Vec::with_capacity(8),
        }
    }

    pub fn with_quantiles<'a, T>(quantiles: T) -> Self
    where
        T: IntoIterator<Item = &'a Quantile>,
    {
        let mut stats = HistogramStats::new();
        for q in quantiles {
            stats.quantile.push(HistogramQuantileStats::new(q.clone()));
        }
        stats
    }

    pub fn with_quantile(mut self, quantile: Quantile) -> Self {
        self.quantile.push(HistogramQuantileStats::new(quantile));
        self
    }

    pub fn update<T: Counter>(&self, histogram: &Histogram<T>) {
        self.min.store(histogram.min(), Ordering::Relaxed);
        self.max.store(histogram.max(), Ordering::Relaxed);
        self.mean.store(histogram.mean(), Ordering::Relaxed);
        for q in &self.quantile {
            q.value.store(
                histogram.value_at_quantile(q.quantile.value()),
                Ordering::Relaxed,
            );
        }
    }

    pub fn foreach_stat<F>(&self, mut call: F)
    where
        F: FnMut(Option<f64>, &str, f64),
    {
        let min = self.min.load(Ordering::Relaxed);
        call(None, "min", min as f64);
        let max = self.max.load(Ordering::Relaxed);
        call(None, "max", max as f64);
        let mean = self.mean.load(Ordering::Relaxed);
        call(None, "mean", mean);
        for q in &self.quantile {
            let v = q.value.load(Ordering::Relaxed);
            call(Some(q.quantile.value()), q.quantile.as_str(), v as f64);
        }
    }
}

impl Default for HistogramStats {
    fn default() -> Self {
        HistogramStats::new()
            .with_quantile(Quantile::PCT50)
            .with_quantile(Quantile::PCT80)
            .with_quantile(Quantile::PCT90)
            .with_quantile(Quantile::PCT95)
            .with_quantile(Quantile::PCT99)
    }
}
