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

mod buf;
mod dns;
mod egress;
mod error;
mod haproxy;
mod host;
mod pool;
mod port;
mod proxy;
mod rate_limit;
mod socks;
mod tcp;
mod tls;
mod tlv;
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
pub use pool::ConnectionPoolConfig;
pub use port::{PortRange, Ports};
pub use proxy::{Proxy, ProxyParseError, ProxyRequestType, Socks4Proxy, Socks5Proxy};
pub use rate_limit::{
    TcpSockSpeedLimitConfig, UdpSockSpeedLimitConfig, RATE_LIMIT_SHIFT_MILLIS_DEFAULT,
    RATE_LIMIT_SHIFT_MILLIS_MAX,
};
pub use socks::SocksAuth;
pub use tcp::*;
pub use tls::*;
pub use tlv::{T1L2BVParse, TlvParse};
pub use udp::{UdpListenConfig, UdpMiscSockOpts};
pub use upstream::{UpstreamAddr, UpstreamHostRef, WeightedUpstreamAddr};

#[cfg(unix)]
pub use interface::InterfaceName;

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
