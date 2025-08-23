/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod encryption;

pub use encryption::DnsEncryptionProtocol;
#[cfg(feature = "rustls")]
pub use encryption::{DnsEncryptionConfig, DnsEncryptionConfigBuilder};
