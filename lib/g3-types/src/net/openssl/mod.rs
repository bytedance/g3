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

mod client;
pub use client::{
    OpensslClientConfig, OpensslClientConfigBuilder, OpensslInterceptionClientConfig,
    OpensslInterceptionClientConfigBuilder,
};

mod server;
pub use server::{
    OpensslInterceptionServerConfig, OpensslInterceptionServerConfigBuilder, OpensslServerConfig,
    OpensslServerConfigBuilder, OpensslServerSessionCache, OpensslSessionIdContext,
    OpensslTicketKey, OpensslTicketKeyBuilder,
};

mod cert_pair;
pub use cert_pair::OpensslCertificatePair;

#[cfg(feature = "tongsuo")]
mod tlcp_cert_pair;
#[cfg(feature = "tongsuo")]
pub use tlcp_cert_pair::OpensslTlcpCertificatePair;

mod protocol;
pub use protocol::OpensslProtocol;
