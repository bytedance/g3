/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod auth;
mod collection;
mod datetime;
mod fs;
mod metrics;
mod net;
mod primary;
mod random;
mod rate_limit;
mod speed_limit;

pub use auth::{as_password, as_username};
pub use collection::as_selective_pick_policy;
pub use datetime::as_rfc3339_datetime;
pub use fs::{as_absolute_path, as_config_file_format, as_dir_path, as_file, as_file_path};
pub use metrics::{
    as_metric_node_name, as_metric_tag_name, as_metric_tag_value, as_static_metrics_tags,
    as_weighted_metric_node_name,
};
pub use net::*;
pub use primary::{
    as_ascii, as_bool, as_f64, as_hashmap, as_i32, as_i64, as_list, as_nonzero_i32,
    as_nonzero_isize, as_nonzero_u32, as_nonzero_usize, as_string, as_u8, as_u16, as_u32, as_u64,
    as_usize,
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

#[cfg(feature = "histogram")]
mod histogram;
#[cfg(feature = "histogram")]
pub use histogram::{as_histogram_metrics_config, as_quantile, as_quantile_list};

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
    as_rustls_certificate_pair, as_rustls_certificates, as_rustls_client_config_builder,
    as_rustls_private_key, as_rustls_server_config_builder, as_rustls_server_name,
};

#[cfg(feature = "openssl")]
mod openssl;
#[cfg(feature = "openssl")]
pub use self::openssl::{
    as_openssl_certificate_pair, as_openssl_certificates, as_openssl_private_key,
    as_openssl_tlcp_certificate_pair, as_openssl_tls_server_config_builder,
    as_tls_interception_client_config_builder, as_tls_interception_server_config_builder,
    as_to_many_openssl_tls_client_config_builder, as_to_one_openssl_tls_client_config_builder,
};

#[cfg(feature = "quinn")]
mod quinn;
#[cfg(feature = "quinn")]
pub use quinn::as_quinn_transport_config;

#[cfg(all(
    any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        windows,
    ),
    feature = "sched"
))]
mod sched;
#[cfg(all(
    any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        windows,
    ),
    feature = "sched"
))]
pub use sched::*;

#[cfg(feature = "route")]
mod route;
#[cfg(feature = "route")]
pub use route::*;

#[cfg(feature = "dpi")]
mod dpi;
#[cfg(feature = "dpi")]
pub use dpi::*;

#[cfg(feature = "geoip")]
mod geoip;
#[cfg(feature = "geoip")]
pub use geoip::{as_continent_code, as_ip_location, as_iso_country_code};
