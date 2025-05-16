/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::types::*;

mod reply;
mod request;
mod udp_io;

pub use reply::Socks5Reply;
pub use request::Socks5Request;
pub use udp_io::{SocksUdpHeader, UdpInput, UdpOutput};

pub mod auth;
pub mod client;

#[cfg(feature = "quic")]
mod quic;
#[cfg(feature = "quic")]
pub use quic::Socks5UdpTokioRuntime;
