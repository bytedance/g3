/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use std::io;
use std::sync::Arc;

use async_trait::async_trait;
use hickory_proto::iocompat::{AsyncIoStdAsTokio, AsyncIoTokioAsStd};
use hickory_proto::tcp::Connect;
use rustls::{ClientConfig, ServerName};
use tokio_rustls::client::TlsStream;
use tokio_rustls::TlsConnector;

use super::TlsConnect;

pub struct RustlsConnector {
    pub config: Arc<ClientConfig>,
    pub tls_name: ServerName,
}

#[async_trait]
impl<S: Connect> TlsConnect<S> for RustlsConnector {
    type TlsStream = AsyncIoTokioAsStd<TlsStream<AsyncIoStdAsTokio<S>>>;

    fn server_name(&self) -> String {
        match &self.tls_name {
            ServerName::DnsName(domain) => domain.as_ref().to_string(),
            ServerName::IpAddress(ip) => ip.to_string(),
            _ => unreachable!(),
        }
    }

    async fn tls_connect(&self, stream: S) -> io::Result<Self::TlsStream> {
        let connector = TlsConnector::from(self.config.clone());
        connector
            .connect(self.tls_name.clone(), AsyncIoStdAsTokio(stream))
            .await
            .map(|s| AsyncIoTokioAsStd(s))
    }
}
