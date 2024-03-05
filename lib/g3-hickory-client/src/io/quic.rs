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

use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use bytes::BytesMut;
use futures_util::{FutureExt, Stream};
use hickory_proto::error::{ProtoError, ProtoErrorKind};
use hickory_proto::op::Message;
use hickory_proto::serialize::binary::{BinEncodable, BinEncoder};
use hickory_proto::xfer::{DnsRequest, DnsRequestSender, DnsResponse, DnsResponseStream};
use quinn::{Connection, Endpoint, EndpointConfig, RecvStream, TokioRuntime, VarInt};
use rustls::ClientConfig;

pub async fn connect_with_bind_addr(
    name_server: SocketAddr,
    bind_addr: Option<SocketAddr>,
    mut tls_config: ClientConfig,
    tls_name: &str,
) -> Result<QuicClientStream, ProtoError> {
    let bind_addr = bind_addr.unwrap_or_else(|| match name_server {
        SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        SocketAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
    });
    let sock = UdpSocket::bind(bind_addr)?;
    sock.connect(name_server)?;

    let endpoint_config = EndpointConfig::default(); // TODO set max payload size
    let mut endpoint = Endpoint::new(endpoint_config, None, sock, Arc::new(TokioRuntime))?;

    if tls_config.alpn_protocols.is_empty() {
        tls_config.alpn_protocols = vec![b"doq".to_vec()];
    }
    let quinn_config = quinn::ClientConfig::new(Arc::new(tls_config));
    // TODO set transport config
    endpoint.set_default_client_config(quinn_config);

    let connection = endpoint.connect(name_server, tls_name)?.await?;

    Ok(QuicClientStream::new(connection))
}

/// A future that resolves to an QuicClientStream
pub struct QuicClientConnect(
    Pin<Box<dyn Future<Output = Result<QuicClientStream, ProtoError>> + Send>>,
);

impl Future for QuicClientConnect {
    type Output = Result<QuicClientStream, ProtoError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.0.poll_unpin(cx)
    }
}

/// A DNS client connection for DNS-over-QUIC
#[must_use = "futures do nothing unless polled"]
pub struct QuicClientStream {
    quic_connection: Connection,
    is_shutdown: bool,
}

impl QuicClientStream {
    pub fn new(connection: Connection) -> Self {
        QuicClientStream {
            quic_connection: connection,
            is_shutdown: false,
        }
    }
}

impl DnsRequestSender for QuicClientStream {
    /// The send loop for QUIC in DNS stipulates that a new QUIC "stream" should be opened and use for sending data.
    ///
    /// It should be closed after receiving the response. TODO: AXFR/IXFR support...
    fn send_message(&mut self, mut message: DnsRequest) -> DnsResponseStream {
        if self.is_shutdown {
            panic!("can not send messages after stream is shutdown")
        }

        // per the RFC, the DNS Message ID MUST be set to zero
        message.set_id(0);

        Box::pin(quic_send_recv(self.quic_connection.clone(), message)).into()
    }

    fn shutdown(&mut self) {
        self.is_shutdown = true;
        // no error
        self.quic_connection.close(VarInt::from_u32(0), b"Shutdown");
    }

    fn is_shutdown(&self) -> bool {
        self.is_shutdown
    }
}

impl Stream for QuicClientStream {
    type Item = Result<(), ProtoError>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.is_shutdown {
            Poll::Ready(None)
        } else {
            Poll::Ready(Some(Ok(())))
        }
    }
}

async fn quic_send_recv(
    connection: Connection,
    message: DnsRequest,
) -> Result<DnsResponse, ProtoError> {
    let message = message.into_parts().0;
    let (mut send_stream, recv_stream) = connection.open_bi().await?;

    // prepare the buffer
    let mut buffer = Vec::with_capacity(512);
    buffer.push(b'\0');
    buffer.push(b'\0');
    {
        let mut encoder = BinEncoder::new(&mut buffer);
        message.emit(&mut encoder)?;
    }
    let message_len = buffer.len() - 2;
    let message_len = u16::try_from(message_len)
        .map_err(|_| ProtoErrorKind::MaxBufferSizeExceeded(message_len))?;
    let len = message_len.to_be_bytes();
    buffer[0] = len[0];
    buffer[1] = len[1];

    send_stream.write_all(&buffer).await?;
    // The client MUST send the DNS query over the selected stream,
    // and MUST indicate through the STREAM FIN mechanism that no further data will be sent on that stream.
    send_stream.finish().await?;

    quic_recv(recv_stream).await
}

async fn quic_recv(mut recv_stream: RecvStream) -> Result<DnsResponse, ProtoError> {
    let mut len_buf = [0u8; 2];
    recv_stream.read_exact(&mut len_buf).await?;
    let message_len = u16::from_be_bytes(len_buf) as usize;

    let mut buffer = BytesMut::with_capacity(message_len);
    buffer.resize(message_len, 0);
    recv_stream.read_exact(&mut buffer).await?;
    let message = Message::from_vec(&buffer)?;
    if message.id() != 0 {
        return Err(ProtoErrorKind::QuicMessageIdNot0(message.id()).into());
    }

    Ok(DnsResponse::new(message, buffer.to_vec()))
}
