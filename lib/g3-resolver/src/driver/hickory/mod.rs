/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

mod config;
pub use config::HickoryDriverConfig;

mod client;
use client::{DnsRequest, HickoryClient, HickoryClientConfig};

mod driver;
use driver::HickoryResolver;

mod error;
