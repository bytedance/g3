/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::time::{Duration, Instant};

pub fn wait_duration(emit_duration: Duration, instant_start: Instant) {
    let instant_now = Instant::now();
    if let Some(instant_next) = instant_start.checked_add(emit_duration) {
        // re-calculate the duration
        if let Some(dur) = instant_next.checked_duration_since(instant_now) {
            std::thread::sleep(dur);
        }
    } else {
        std::thread::sleep(emit_duration);
    }
}
