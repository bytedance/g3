/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::hostname;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::hostname;
