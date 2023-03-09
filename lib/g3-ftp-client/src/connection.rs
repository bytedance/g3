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
