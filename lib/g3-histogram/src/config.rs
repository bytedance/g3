/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use hdrhistogram::Counter;
use tokio::runtime::Handle;

use crate::{HistogramRecorder, HistogramStats, Quantile, RotatingHistogram};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistogramMetricsConfig {
    quantile_list: BTreeSet<Quantile>,
    rotate_interval: Duration,
}

impl HistogramMetricsConfig {
    pub fn with_rotate(dur: Duration) -> Self {
        HistogramMetricsConfig {
            quantile_list: BTreeSet::new(),
            rotate_interval: dur,
        }
    }

    #[inline]
    pub fn set_quantile_list(&mut self, list: BTreeSet<Quantile>) {
        self.quantile_list = list;
    }

    #[inline]
    pub fn set_rotate_interval(&mut self, dur: Duration) {
        self.rotate_interval = dur;
    }

    #[inline]
    pub fn rotate_interval(&self) -> Duration {
        self.rotate_interval
    }

    pub fn build_spawned<T>(
        &self,
        handle: Option<Handle>,
    ) -> (HistogramRecorder<T>, Arc<HistogramStats>)
    where
        T: Counter + Send + 'static,
    {
        let (h, r) = RotatingHistogram::new(self.rotate_interval);
        let stats = if self.quantile_list.is_empty() {
            Arc::new(HistogramStats::default())
        } else {
            Arc::new(HistogramStats::with_quantiles(&self.quantile_list))
        };
        h.spawn_refresh(Arc::clone(&stats), handle);
        (r, stats)
    }
}

impl Default for HistogramMetricsConfig {
    fn default() -> Self {
        HistogramMetricsConfig::with_rotate(Duration::from_secs(4))
    }
}
