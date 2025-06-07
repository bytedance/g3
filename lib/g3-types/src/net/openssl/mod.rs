/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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

mod tlcp_cert_pair;
pub use tlcp_cert_pair::OpensslTlcpCertificatePair;

mod protocol;
pub use protocol::OpensslProtocol;
