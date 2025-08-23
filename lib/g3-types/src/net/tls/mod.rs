/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod alpn;
pub use alpn::{AlpnProtocol, TlsAlpn};

mod server_name;
pub use server_name::TlsServerName;

mod service_type;
pub use service_type::TlsServiceType;

mod cert_usage;
pub use cert_usage::TlsCertUsage;

mod ticket_name;
pub use ticket_name::{TICKET_KEY_NAME_LENGTH, TicketKeyName};

mod ticketer;
pub use ticketer::{
    RollingTicketKey, RollingTicketer, TICKET_AES_IV_LENGTH, TICKET_AES_KEY_LENGTH,
    TICKET_HMAC_KEY_LENGTH,
};

mod version;
pub use version::TlsVersion;

mod alert;
pub use alert::{TlsAlert, TlsAlertType};
