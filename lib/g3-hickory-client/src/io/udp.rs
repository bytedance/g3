/*
 * Copyright 2025 ByteDance and/or its affiliates.
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
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use futures_util::Stream;
use hickory_proto::xfer::{DnsRequest, DnsRequestSender, DnsResponse, DnsResponseStream};
use hickory_proto::{ProtoError, ProtoErrorKind};

/// Max size for the UDP receive buffer as recommended by
/// [RFC6891](https://datatracker.ietf.org/doc/html/rfc6891#section-6.2.5).
const MAX_RECEIVE_BUFFER_SIZE: usize = 4_096;

pub async fn connect(
    name_server: SocketAddr,
    bind_addr: Option<SocketAddr>,
    request_timeout: Duration,
) -> Result<UdpClientStream, ProtoError> {
    Ok(UdpClientStream {
        name_server,
        bind_addr,
        request_timeout,
        is_shutdown: false,
    })
}

/// A UDP client stream of DNS binary packets
#[must_use = "futures do nothing unless polled"]
pub struct UdpClientStream {
    name_server: SocketAddr,
    bind_addr: Option<SocketAddr>,
    request_timeout: Duration,
    is_shutdown: bool,
}

impl DnsRequestSender for UdpClientStream {
    fn send_message(&mut self, message: DnsRequest) -> DnsResponseStream {
        if self.is_shutdown {
            panic!("can not send messages after stream is shutdown")
        }

        Box::pin(timed_udp_send_recv(
            self.name_server,
            self.bind_addr,
            message,
            self.request_timeout,
        ))
        .into()
    }

    fn shutdown(&mut self) {
        self.is_shutdown = true;
    }

    fn is_shutdown(&self) -> bool {
        self.is_shutdown
    }
}

impl Stream for UdpClientStream {
    type Item = Result<(), ProtoError>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.is_shutdown {
            Poll::Ready(None)
        } else {
            Poll::Ready(Some(Ok(())))
        }
    }
}

async fn timed_udp_send_recv(
    name_server: SocketAddr,
    bind_addr: Option<SocketAddr>,
    request: DnsRequest,
    request_timeout: Duration,
) -> Result<DnsResponse, ProtoError> {
    tokio::time::timeout(
        request_timeout,
        udp_send_recv(name_server, bind_addr, request),
    )
    .await
    .map_err(|_| ProtoErrorKind::Timeout)?
}

async fn udp_send_recv(
    name_server: SocketAddr,
    bind_addr: Option<SocketAddr>,
    mut request: DnsRequest,
) -> Result<DnsResponse, ProtoError> {
    // set a random ID
    let id = fastrand::u16(..);
    request.set_id(id);

    let socket = crate::connect::udp::udp_connect(name_server, bind_addr)?;
    let socket = tokio::net::UdpSocket::from_std(socket)?;

    let bytes = request.to_vec()?;
    let nw = socket.send(&bytes).await?;
    if nw != bytes.len() {
        return Err(ProtoError::from(format!(
            "Not all bytes of message sent, {nw} of {}",
            bytes.len()
        )));
    }

    loop {
        let mut recv_buf = vec![0; MAX_RECEIVE_BUFFER_SIZE];

        let nr = socket.recv(&mut recv_buf).await?;
        recv_buf.resize(nr, 0);
        let response = DnsResponse::from_buffer(recv_buf)?;
        if response.id() != id {
            continue;
        }

        if !response
            .queries()
            .iter()
            .all(|rq| request.queries().contains(rq))
        {
            continue;
        }

        return Ok(response);
    }
}
