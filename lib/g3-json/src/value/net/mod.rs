/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod base;
mod ports;
mod proxy;
mod tcp;
mod tls;
mod udp;

#[cfg(feature = "http")]
mod http;

pub use base::{as_domain, as_egress_area, as_host, as_ipaddr, as_upstream_addr};
pub use ports::as_ports;
pub use proxy::as_proxy_request_type;
pub use tcp::{as_tcp_connect_config, as_tcp_keepalive_config, as_tcp_misc_sock_opts};
pub use tls::as_tls_version;
pub use udp::as_udp_misc_sock_opts;

#[cfg(feature = "acl-rule")]
pub use base::as_ip_network;

#[cfg(feature = "http")]
pub use http::as_http_keepalive_config;
