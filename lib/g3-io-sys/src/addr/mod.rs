/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub(super) use unix::RawSocketAddr;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(super) use windows::RawSocketAddr;
