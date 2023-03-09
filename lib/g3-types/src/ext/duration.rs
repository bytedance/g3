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
