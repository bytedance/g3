/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

use hickory_net::NetError;
use hickory_net::runtime::{DnsTcpStream, iocompat::AsyncIoTokioAsStd};
use hickory_net::tcp::{TcpClientStream, TcpStream};
use hickory_net::xfer::StreamReceiver;

use rustls::ClientConfig;
use rustls_pki_types::ServerName;

use g3_socket::TcpConnectInfo;

pub async fn connect(
    connect_info: TcpConnectInfo,
    tls_config: ClientConfig,
    tls_name: ServerName<'static>,
    outbound_messages: StreamReceiver,
    connect_timeout: Duration,
) -> Result<TcpClientStream<impl DnsTcpStream>, NetError> {
    let tls_stream = tokio::time::timeout(
        connect_timeout,
        crate::connect::rustls::tls_connect(&connect_info, tls_config, tls_name, b"dot"),
    )
    .await
    .map_err(|_| NetError::Timeout)??;

    let stream = TcpStream::from_stream_with_receiver(
        AsyncIoTokioAsStd(tls_stream),
        connect_info.server,
        outbound_messages,
    );
    Ok(TcpClientStream::from_stream(stream))
}
