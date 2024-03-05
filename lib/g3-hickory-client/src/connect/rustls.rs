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

use std::net::SocketAddr;
use std::sync::Arc;

use hickory_proto::error::ProtoError;
use rustls::{ClientConfig, ServerName};
use tokio::net::{TcpSocket, TcpStream};
use tokio_rustls::client::TlsStream;
use tokio_rustls::TlsConnector;

pub(crate) async fn tls_connect(
    name_server: SocketAddr,
    bind_addr: Option<SocketAddr>,
    mut tls_config: ClientConfig,
    tls_name: ServerName,
    alpn_protocol: &'static [u8],
) -> Result<TlsStream<TcpStream>, ProtoError> {
    let socket = match name_server {
        SocketAddr::V4(_) => TcpSocket::new_v4(),
        SocketAddr::V6(_) => TcpSocket::new_v6(),
    }?;
    if let Some(addr) = bind_addr {
        socket.bind(addr)?;
    }

    if tls_config.alpn_protocols.is_empty() {
        tls_config.alpn_protocols = vec![alpn_protocol.to_vec()];
    }

    let tcp_stream = socket.connect(name_server).await?;
    let tls_connector = TlsConnector::from(Arc::new(tls_config));
    let tls_stream = tls_connector.connect(tls_name, tcp_stream).await?;

    Ok(tls_stream)
}
