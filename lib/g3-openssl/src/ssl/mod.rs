/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
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

mod lazy_accept;
pub use lazy_accept::SslLazyAcceptor;

#[cfg_attr(not(feature = "async-job"), path = "connect.rs")]
#[cfg_attr(feature = "async-job", path = "async_connect.rs")]
mod connect;
pub use connect::SslConnector;
