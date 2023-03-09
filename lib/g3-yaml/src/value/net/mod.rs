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

mod base;
mod buf;
mod haproxy;
mod port;
mod tcp;
mod udp;

#[cfg(feature = "http")]
mod http;

#[cfg(feature = "ftp-client")]
mod ftp;

#[cfg(feature = "rustls")]
mod dns;

#[cfg(feature = "proxy")]
mod proxy;

pub use base::{
    as_domain, as_host, as_ipaddr, as_ipv4addr, as_ipv6addr, as_sockaddr, as_upstream_addr, as_url,
    as_weighted_sockaddr, as_weighted_upstream_addr,
};
pub use buf::as_socket_buffer_config;
pub use haproxy::as_proxy_protocol_version;
pub use port::{as_port_range, as_ports};
pub use tcp::{
    as_happy_eyeballs_config, as_tcp_connect_config, as_tcp_keepalive_config, as_tcp_listen_config,
    as_tcp_misc_sock_opts,
};
pub use udp::as_udp_misc_sock_opts;

#[cfg(feature = "acl-rule")]
pub use base::as_ip_network;

#[cfg(feature = "http")]
pub use self::http::{
    as_http_forward_capability, as_http_forwarded_header_type, as_http_header_name,
    as_http_keepalive_config, as_http_server_id,
};

#[cfg(feature = "ftp-client")]
pub use ftp::as_ftp_client_config;

#[cfg(feature = "rustls")]
pub use dns::as_dns_encryption_protocol_builder;

#[cfg(feature = "proxy")]
pub use proxy::as_proxy_request_type;
