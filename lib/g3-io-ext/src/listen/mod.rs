/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod tcp;
pub use tcp::LimitedTcpListener;

#[cfg(feature = "rustls")]
mod tls;
#[cfg(feature = "rustls")]
pub use tls::LimitedTlsListener;
