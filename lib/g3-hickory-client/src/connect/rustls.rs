/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
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
