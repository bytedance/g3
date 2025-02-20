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

mod cache;
mod io;
mod limit;
mod listen;
mod time;
mod udp;

pub use cache::{
    EffectiveCacheData, EffectiveCacheHandle, EffectiveCacheRuntime, EffectiveQueryHandle,
    create_effective_cache,
};
pub use io::*;
pub use limit::*;
pub use listen::*;
pub use time::*;
pub use udp::*;

pub mod haproxy;

#[cfg(feature = "quic")]
mod quic;
#[cfg(feature = "quic")]
pub use quic::*;

#[cfg(feature = "openssl")]
pub use io::stream::openssl;

#[cfg(feature = "rustls")]
pub use io::stream::rustls;
