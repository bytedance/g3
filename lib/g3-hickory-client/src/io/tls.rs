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

use std::time::Duration;

use hickory_proto::runtime::iocompat::AsyncIoTokioAsStd;
use hickory_proto::tcp::{DnsTcpStream, TcpClientStream, TcpStream};
use hickory_proto::xfer::StreamReceiver;
use hickory_proto::ProtoError;
use rustls::ClientConfig;
use rustls_pki_types::ServerName;

use g3_socket::TcpConnectInfo;

pub async fn connect(
    connect_info: TcpConnectInfo,
    tls_config: ClientConfig,
    tls_name: ServerName<'static>,
    outbound_messages: StreamReceiver,
    connect_timeout: Duration,
) -> Result<TcpClientStream<impl DnsTcpStream>, ProtoError> {
    let tls_stream = tokio::time::timeout(
        connect_timeout,
        crate::connect::rustls::tls_connect(&connect_info, tls_config, tls_name, b"dot"),
    )
    .await
    .map_err(|_| ProtoError::from("tls connect timed out"))??;

    let stream = TcpStream::from_stream_with_receiver(
        AsyncIoTokioAsStd(tls_stream),
        connect_info.server,
        outbound_messages,
    );
    Ok(TcpClientStream::from_stream(stream))
}
