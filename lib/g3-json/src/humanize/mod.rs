/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod size;
mod time;

pub use size::{as_u64, as_usize};
pub use time::as_duration;
