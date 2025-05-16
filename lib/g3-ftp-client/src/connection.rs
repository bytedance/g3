/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::error::Error;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

use g3_types::net::UpstreamAddr;

#[async_trait]
pub trait FtpConnectionProvider<T: AsyncRead + AsyncWrite, E: Error, UD> {
    async fn new_control_connection(
        &mut self,
        upstream: &UpstreamAddr,
        user_data: &UD,
    ) -> Result<T, E>;
    async fn new_data_connection(
        &mut self,
        server_addr: &UpstreamAddr,
        user_data: &UD,
    ) -> Result<T, E>;
}
