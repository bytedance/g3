/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod driver;
use driver::CAresResolver;

mod config;
pub use config::CAresDriverConfig;

mod error;
