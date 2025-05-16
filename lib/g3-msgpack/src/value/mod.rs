/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod datetime;
mod metrics;
mod net;
mod primary;
mod tls;
mod uuid;

pub use self::uuid::as_uuid;
pub use datetime::as_rfc3339_datetime;
pub use metrics::{as_metrics_name, as_weighted_metrics_name};
pub use net::*;
pub use primary::{as_f64, as_string, as_u32, as_weighted_name_string};
pub use tls::{as_tls_cert_usage, as_tls_service_type};

#[cfg(feature = "openssl")]
mod openssl;
#[cfg(feature = "openssl")]
pub use openssl::{as_openssl_certificate, as_openssl_certificates, as_openssl_private_key};

#[cfg(feature = "rustls")]
mod rustls;
#[cfg(feature = "rustls")]
pub use self::rustls::{as_rustls_certificates, as_rustls_private_key};

#[cfg(feature = "geoip")]
mod geoip;
#[cfg(feature = "geoip")]
pub use geoip::{as_continent_code, as_ip_location, as_iso_country_code};
