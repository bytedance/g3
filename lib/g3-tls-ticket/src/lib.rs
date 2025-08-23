/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

mod config;
pub use config::TlsTicketConfig;

mod source;
use source::TicketSourceConfig;

mod update;
use update::TicketKeyUpdate;
