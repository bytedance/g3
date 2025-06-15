/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod cache;
mod limit;
mod listen;
mod stream;
mod time;
mod udp;

pub use cache::{
    EffectiveCacheData, EffectiveCacheHandle, EffectiveCacheRuntime, EffectiveQueryHandle,
    create_effective_cache,
};
pub use limit::*;
pub use listen::*;
pub use stream::*;
pub use time::*;
pub use udp::*;

pub mod haproxy;

#[cfg(feature = "quic")]
mod quic;
#[cfg(feature = "quic")]
pub use quic::*;

#[cfg(feature = "openssl")]
pub use stream::openssl;

#[cfg(feature = "rustls")]
pub use stream::rustls;
