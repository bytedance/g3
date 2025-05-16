/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod sink;
use sink::StatsdMetricsSink;

mod client;
pub use client::StatsdClient;

mod tag;
pub use tag::StatsdTagGroup;

mod config;
pub use config::{StatsdBackend, StatsdClientConfig};
