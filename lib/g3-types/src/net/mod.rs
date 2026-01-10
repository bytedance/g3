/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod buf;
mod dns;
mod egress;
mod error;
mod haproxy;
mod host;
mod ldap;
mod pool;
mod port;
mod proxy;
mod quic;
mod rate_limit;
mod socks;
mod tcp;
mod tls;
mod udp;
mod upstream;

#[cfg(unix)]
mod interface;

#[cfg(feature = "http")]
mod http;
#[cfg(feature = "http")]
mod websocket;

#[cfg(feature = "rustls")]
mod rustls;

#[cfg(feature = "openssl")]
mod openssl;

#[cfg(feature = "quinn")]
mod quinn;

pub use buf::SocketBufferConfig;
pub use dns::*;
pub use egress::{EgressArea, EgressInfo};
pub use error::ConnectError;
pub use haproxy::{
    ProxyProtocolEncodeError, ProxyProtocolEncoder, ProxyProtocolV2Encoder, ProxyProtocolVersion,
};
pub use host::Host;
pub use ldap::*;
pub use pool::ConnectionPoolConfig;
pub use port::{PortRange, Ports};
pub use proxy::{Proxy, ProxyParseError, ProxyRequestType, Socks4Proxy, Socks5Proxy};
pub use quic::*;
pub use rate_limit::{
    RATE_LIMIT_SHIFT_MILLIS_DEFAULT, RATE_LIMIT_SHIFT_MILLIS_MAX, TcpSockSpeedLimitConfig,
    UdpSockSpeedLimitConfig,
};
pub use socks::SocksAuth;
pub use tcp::*;
pub use tls::*;
pub use udp::{UdpListenConfig, UdpMiscSockOpts};
pub use upstream::{UpstreamAddr, UpstreamHostRef, WeightedUpstreamAddr};

#[cfg(unix)]
pub use interface::Interface;

#[cfg(feature = "http")]
pub use self::http::*;
#[cfg(feature = "http")]
pub use websocket::*;

#[cfg(feature = "http")]
pub use proxy::HttpProxy;

#[cfg(feature = "rustls")]
pub use self::rustls::*;

#[cfg(feature = "openssl")]
pub use self::openssl::*;

#[cfg(feature = "quinn")]
pub use self::quinn::*;
