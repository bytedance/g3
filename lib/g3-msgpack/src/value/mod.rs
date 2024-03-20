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

mod datetime;
mod metrics;
mod primary;
mod tls;
mod uuid;

#[cfg(feature = "openssl")]
mod openssl;

#[cfg(feature = "rustls")]
mod rustls;

pub use self::uuid::as_uuid;
pub use datetime::as_rfc3339_datetime;
pub use metrics::{as_metrics_name, as_weighted_metrics_name};
pub use primary::{as_f64, as_string, as_u32, as_weighted_name_string};
pub use tls::as_tls_service_type;

pub use openssl::{as_openssl_certificates, as_openssl_private_key};

#[cfg(feature = "rustls")]
pub use self::rustls::{as_rustls_certificates, as_rustls_private_key};
