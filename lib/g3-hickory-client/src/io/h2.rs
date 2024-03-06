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
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::{Buf, Bytes};
use futures_util::Stream;
use h2::client::SendRequest;
use hickory_proto::error::{ProtoError, ProtoErrorKind};
use hickory_proto::xfer::{DnsRequest, DnsRequestSender, DnsResponse, DnsResponseStream};
use http::{Response, Version};
use rustls::{ClientConfig, ServerName};

use super::http::request::HttpDnsRequestBuilder;
use super::http::response::HttpDnsResponse;

pub async fn connect(
    name_server: SocketAddr,
    bind_addr: Option<SocketAddr>,
    tls_config: ClientConfig,
    tls_name: ServerName,
    connect_timeout: Duration,
    request_timeout: Duration,
) -> Result<HttpsClientStream, ProtoError> {
    let server_name = match &tls_name {
        ServerName::DnsName(domain) => domain.as_ref().to_string(),
        ServerName::IpAddress(ip) => ip.to_string(),
        _ => {
            return Err(ProtoError::from(format!(
                "unsupported tls name: {:?}",
                tls_name
            )))
        }
    };

    let tls_stream = tokio::time::timeout(
        connect_timeout,
        crate::connect::rustls::tls_connect(name_server, bind_addr, tls_config, tls_name, b"h2"),
    )
    .await
    .map_err(|_| ProtoError::from("tls connect timed out"))??;

    let mut client_builder = h2::client::Builder::new();
    client_builder.enable_push(false);

    let (send_request, connection) = client_builder
        .handshake(tls_stream)
        .await
        .map_err(|e| format!("h2 handshake error: {e}"))?;

    tokio::spawn(async move {
        let _ = connection.await;
    });

    HttpsClientStream::new(&server_name, send_request, request_timeout)
}

/// A DNS client connection for DNS-over-HTTPS
#[derive(Clone)]
#[must_use = "futures do nothing unless polled"]
pub struct HttpsClientStream {
    request_builder: Arc<HttpDnsRequestBuilder>,
    request_timeout: Duration,
    h2: SendRequest<Bytes>,
    is_shutdown: bool,
}

impl HttpsClientStream {
    pub fn new(
        name_server_name: &str,
        h2: SendRequest<Bytes>,
        request_timeout: Duration,
    ) -> Result<Self, ProtoError> {
        let request_builder = HttpDnsRequestBuilder::new(Version::HTTP_2, name_server_name)?;
        Ok(HttpsClientStream {
            request_builder: Arc::new(request_builder),
            request_timeout,
            h2,
            is_shutdown: false,
        })
    }
}

impl DnsRequestSender for HttpsClientStream {
    /// This indicates that the HTTP message was successfully sent, and we now have the response.RecvStream
    ///
    /// If the request fails, this will return the error, and it should be assumed that the Stream portion of
    ///   this will have no date.
    fn send_message(&mut self, mut message: DnsRequest) -> DnsResponseStream {
        if self.is_shutdown {
            panic!("can not send messages after stream is shutdown")
        }

        // per the RFC, a zero id allows for the HTTP packet to be cached better
        message.set_id(0);

        let bytes = match message.to_vec() {
            Ok(bytes) => bytes,
            Err(err) => return err.into(),
        };

        Box::pin(timed_h2_send_recv(
            self.h2.clone(),
            Bytes::from(bytes),
            Arc::clone(&self.request_builder),
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

impl Stream for HttpsClientStream {
    type Item = Result<(), ProtoError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.is_shutdown {
            return Poll::Ready(None);
        }

        // just checking if the connection is ok
        match self.h2.poll_ready(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Some(Ok(()))),
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(e)) => Poll::Ready(Some(Err(ProtoError::from(format!(
                "h2 stream errored: {e}",
            ))))),
        }
    }
}

async fn timed_h2_send_recv(
    h2: SendRequest<Bytes>,
    message: Bytes,
    request_builder: Arc<HttpDnsRequestBuilder>,
    request_timeout: Duration,
) -> Result<DnsResponse, ProtoError> {
    tokio::time::timeout(request_timeout, h2_send_recv(h2, message, request_builder))
        .await
        .map_err(|_| ProtoErrorKind::Timeout)?
}

async fn h2_send_recv(
    h2: SendRequest<Bytes>,
    message: Bytes,
    request_builder: Arc<HttpDnsRequestBuilder>,
) -> Result<DnsResponse, ProtoError> {
    let mut h2 = h2
        .ready()
        .await
        .map_err(|e| format!("h2 wait send_request error: {e}"))?;

    // build up the http request
    let request = request_builder.post(message.remaining());

    // Send the request
    let (response_future, mut send_stream) = h2
        .send_request(request, false)
        .map_err(|e| format!("h2 send_request error: {e}"))?;
    send_stream
        .send_data(message, true)
        .map_err(|e| format!("h2 send_data error: {e}"))?;

    let response_stream = response_future
        .await
        .map_err(|e| format!("received a stream error: {e}"))?;
    let (parts, mut recv_stream) = response_stream.into_parts();

    let mut rsp = HttpDnsResponse::new(Response::from_parts(parts, ()))?;

    while let Some(partial_bytes) = recv_stream.data().await {
        let partial_bytes = partial_bytes.map_err(|e| format!("bad http request: {e}"))?;

        rsp.push_body(partial_bytes);
        if rsp.body_end() {
            break;
        }
    }

    rsp.into_dns_response()
}
