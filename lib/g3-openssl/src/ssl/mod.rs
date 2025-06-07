/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod error;
pub use error::SslError;
use error::{ConvertSslError, SslErrorAction};

mod wrapper;
use wrapper::SslIoWrapper;

#[cfg(feature = "async-job")]
mod async_mode;
#[cfg(feature = "async-job")]
use async_mode::AsyncEnginePoller;
#[cfg(feature = "async-job")]
pub use async_mode::SslAsyncModeExt;

mod stream;
pub use stream::SslStream;

#[cfg_attr(not(feature = "async-job"), path = "accept.rs")]
#[cfg_attr(feature = "async-job", path = "async_accept.rs")]
mod accept;
pub use accept::SslAcceptor;

#[cfg(not(libressl))]
mod lazy_accept;
#[cfg(not(libressl))]
pub use lazy_accept::SslLazyAcceptor;

#[cfg_attr(not(feature = "async-job"), path = "connect.rs")]
#[cfg_attr(feature = "async-job", path = "async_connect.rs")]
mod connect;
pub use connect::SslConnector;

mod types;
pub use types::SslInfoCallbackWhere;
