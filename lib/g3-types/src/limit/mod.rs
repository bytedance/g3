/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod gauge_semaphore;
pub use gauge_semaphore::{GaugeSemaphore, GaugeSemaphoreAcquireError, GaugeSemaphorePermit};

mod stream_speed;
pub use stream_speed::GlobalStreamSpeedLimitConfig;

mod datagram_speed;
pub use datagram_speed::GlobalDatagramSpeedLimitConfig;

mod rate;
pub use rate::*;
