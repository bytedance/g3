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

use std::sync::Arc;

use hickory_proto::ProtoError;
use rustls::ClientConfig;
use rustls_pki_types::ServerName;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;

use g3_socket::TcpConnectInfo;

pub(crate) async fn tls_connect(
    connect_info: &TcpConnectInfo,
    mut tls_config: ClientConfig,
    tls_name: ServerName<'static>,
    alpn_protocol: &'static [u8],
) -> Result<TlsStream<TcpStream>, ProtoError> {
    let tcp_stream = connect_info.tcp_connect().await?;

    if tls_config.alpn_protocols.is_empty() {
        tls_config.alpn_protocols = vec![alpn_protocol.to_vec()];
    }

    let tls_connector = TlsConnector::from(Arc::new(tls_config));
    let tls_stream = tls_connector.connect(tls_name, tcp_stream).await?;

    Ok(tls_stream)
}
