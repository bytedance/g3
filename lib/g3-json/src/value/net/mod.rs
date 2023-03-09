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
mod ports;
mod tcp;
mod udp;

#[cfg(feature = "http")]
mod http;

#[cfg(feature = "proxy")]
mod proxy;

pub use base::{as_domain, as_egress_area, as_host, as_ipaddr, as_upstream_addr};
pub use ports::as_ports;
pub use tcp::{as_tcp_connect_config, as_tcp_keepalive_config, as_tcp_misc_sock_opts};
pub use udp::as_udp_misc_sock_opts;

#[cfg(feature = "acl-rule")]
pub use base::as_ip_network;

#[cfg(feature = "http")]
pub use http::as_http_keepalive_config;

#[cfg(feature = "proxy")]
pub use proxy::as_proxy_request_type;
