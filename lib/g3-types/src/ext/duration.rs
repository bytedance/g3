/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

const NANOS_PER_MILLI: u32 = 1_000_000;

pub trait DurationExt {
    fn as_millis_f64(&self) -> f64;

    fn as_nanos_u64(&self) -> u64;
}

impl DurationExt for Duration {
    fn as_millis_f64(&self) -> f64 {
        (self.as_secs() * 1000) as f64 + (self.subsec_nanos() as f64 / NANOS_PER_MILLI as f64)
    }

    fn as_nanos_u64(&self) -> u64 {
        u64::try_from(self.as_nanos()).unwrap_or(u64::MAX)
    }
}
