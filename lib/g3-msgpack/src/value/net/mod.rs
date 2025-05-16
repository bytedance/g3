/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

mod base;

pub use base::as_ipaddr;

#[cfg(feature = "geoip")]
pub use base::as_ip_network;
