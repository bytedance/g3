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

mod alpn;
pub use alpn::{AlpnProtocol, TlsAlpn};

mod server_name;
pub use server_name::TlsServerName;

mod service_type;
pub use service_type::TlsServiceType;

mod cert_usage;
pub use cert_usage::TlsCertUsage;

mod ticket_name;
pub use ticket_name::{TicketKeyName, TICKET_KEY_NAME_LENGTH};

mod ticketer;
pub use ticketer::{
    RollingTicketKey, RollingTicketer, TICKET_AES_KEY_LENGTH, TICKET_HMAC_KEY_LENGTH,
};

mod version;
pub use version::TlsVersion;
