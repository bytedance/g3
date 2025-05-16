/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod client;
#[cfg(feature = "quinn")]
pub use client::RustlsQuicClientConfig;
pub use client::{RustlsClientConfig, RustlsClientConfigBuilder};

mod server;
#[cfg(feature = "quinn")]
pub use server::RustlsQuicServerConfig;
pub use server::{RustlsServerConfig, RustlsServerConfigBuilder};

mod cache;
use cache::RustlsServerSessionCache;

mod ticketer;
pub use ticketer::RustlsNoSessionTicketer;

mod cert_pair;
pub use cert_pair::{RustlsCertificatePair, RustlsCertificatePairBuilder};

mod cert_resolver;
pub use cert_resolver::MultipleCertResolver;

mod ca_certs;
pub use ca_certs::load_native_certs_for_rustls;

mod ext;
pub use ext::{
    RustlsClientConnectionExt, RustlsConnectionExt, RustlsServerConfigExt,
    RustlsServerConnectionExt,
};
