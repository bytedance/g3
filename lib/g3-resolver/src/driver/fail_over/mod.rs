/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod config;
pub use config::{FailOverDriverConfig, FailOverDriverStaticConfig};

mod driver;
use driver::FailOverResolver;
