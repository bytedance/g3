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

mod auth;
mod datetime;
mod metrics;
mod net;
mod primary;
mod random;
mod rate_limit;
mod speed_limit;

pub use auth::{as_password, as_username};
pub use datetime::as_rfc3339_datetime;
pub use metrics::{as_metric_node_name, as_weighted_metric_node_name};
pub use net::*;
pub use primary::{
    as_ascii, as_bool, as_bytes, as_f64, as_hashmap, as_i32, as_list, as_nonzero_u32, as_string,
    as_u8, as_u16, as_u32, as_usize,
};
pub use random::as_random_ratio;
pub use rate_limit::as_rate_limit_quota;
pub use speed_limit::{
    as_global_datagram_speed_limit, as_global_stream_speed_limit, as_tcp_sock_speed_limit,
    as_udp_sock_speed_limit,
};

#[cfg(feature = "acl-rule")]
pub mod acl;
#[cfg(feature = "acl-rule")]
pub mod acl_set;

#[cfg(feature = "regex")]
mod regex;
#[cfg(feature = "regex")]
pub use regex::as_regex;

#[cfg(feature = "resolve")]
mod resolve;
#[cfg(feature = "resolve")]
pub use resolve::{as_resolve_redirection_builder, as_resolve_strategy};

#[cfg(feature = "rustls")]
mod rustls;
#[cfg(feature = "rustls")]
pub use self::rustls::{
    as_rustls_client_config_builder, as_rustls_server_config_builder, as_rustls_server_name,
};

#[cfg(feature = "openssl")]
mod openssl;
#[cfg(feature = "tongsuo")]
pub use self::openssl::as_openssl_tlcp_certificate_pair;
#[cfg(feature = "openssl")]
pub use self::openssl::{
    as_openssl_certificate_pair, as_openssl_certificates, as_openssl_private_key,
    as_to_many_openssl_tls_client_config_builder, as_to_one_openssl_tls_client_config_builder,
};

#[cfg(feature = "route")]
mod route;
#[cfg(feature = "route")]
pub use route::*;

#[cfg(feature = "histogram")]
mod histogram;
#[cfg(feature = "histogram")]
pub use histogram::{as_histogram_metrics_config, as_quantile, as_quantile_list};
