/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

mod optional_interval;
pub use optional_interval::OptionalInterval;

mod idle;
pub use idle::{IdleCheck, IdleForceQuitReason, IdleInterval, IdleWheel};
