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
use h3::client::{Connection, SendRequest};
use hickory_proto::error::{ProtoError, ProtoErrorKind};
use hickory_proto::xfer::{DnsRequest, DnsRequestSender, DnsResponse, DnsResponseStream};
use http::Version;
use rustls::ClientConfig;

use super::http::request::HttpDnsRequestBuilder;
use super::http::response::HttpDnsResponse;

pub async fn connect(
    name_server: SocketAddr,
    bind_addr: Option<SocketAddr>,
    tls_config: ClientConfig,
    tls_name: String,
    connect_timeout: Duration,
    request_timeout: Duration,
) -> Result<H3ClientStream, ProtoError> {
    let connection = tokio::time::timeout(
        connect_timeout,
        crate::connect::quinn::quic_connect(name_server, bind_addr, tls_config, &tls_name, b"h3"),
    )
    .await
    .map_err(|_| ProtoError::from("quic connect timed out"))??;

    let h3_connection = h3_quinn::Connection::new(connection);
    let (driver, send_request) = h3::client::new(h3_connection)
        .await
        .map_err(|e| format!("h3 connection failed: {e}"))?;

    H3ClientStream::new(&tls_name, driver, send_request, request_timeout)
}

/// A DNS client connection for DNS-over-HTTP/3
#[must_use = "futures do nothing unless polled"]
pub struct H3ClientStream {
    request_builder: Arc<HttpDnsRequestBuilder>,
    request_timeout: Duration,
    // Corresponds to the dns-name of the HTTP/3 server
    driver: Connection<h3_quinn::Connection, Bytes>,
    send_request: SendRequest<h3_quinn::OpenStreams, Bytes>,
    is_shutdown: bool,
}

impl H3ClientStream {
    pub fn new(
        name_server_name: &str,
        connection: Connection<h3_quinn::Connection, Bytes>,
        send_request: SendRequest<h3_quinn::OpenStreams, Bytes>,
        request_timeout: Duration,
    ) -> Result<Self, ProtoError> {
        let request_builder = HttpDnsRequestBuilder::new(Version::HTTP_3, name_server_name)?;
        Ok(H3ClientStream {
            request_builder: Arc::new(request_builder),
            request_timeout,
            driver: connection,
            send_request,
            is_shutdown: false,
        })
    }
}

impl DnsRequestSender for H3ClientStream {
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

        Box::pin(timed_h3_send_recv(
            self.send_request.clone(),
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

impl Stream for H3ClientStream {
    type Item = Result<(), ProtoError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.is_shutdown {
            return Poll::Ready(None);
        }

        // just checking if the connection is ok
        match self.driver.poll_close(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(e)) => Poll::Ready(Some(Err(ProtoError::from(format!(
                "h3 stream errored: {e}",
            ))))),
        }
    }
}

async fn timed_h3_send_recv(
    h3: SendRequest<h3_quinn::OpenStreams, Bytes>,
    message: Bytes,
    request_builder: Arc<HttpDnsRequestBuilder>,
    request_timeout: Duration,
) -> Result<DnsResponse, ProtoError> {
    tokio::time::timeout(request_timeout, h3_send_recv(h3, message, request_builder))
        .await
        .map_err(|_| ProtoErrorKind::Timeout)?
}

async fn h3_send_recv(
    mut h3: SendRequest<h3_quinn::OpenStreams, Bytes>,
    message: Bytes,
    request_builder: Arc<HttpDnsRequestBuilder>,
) -> Result<DnsResponse, ProtoError> {
    // build up the http request
    let request = request_builder.post(message.remaining());

    let mut send_stream = h3
        .send_request(request)
        .await
        .map_err(|e| format!("h3 send_request error: {e}"))?;
    send_stream
        .send_data(message)
        .await
        .map_err(|e| format!("h3 send_data error: {e}"))?;
    send_stream
        .finish()
        .await
        .map_err(|e| format!("h3 finish send stream error: {e}"))?;

    let response = send_stream
        .recv_response()
        .await
        .map_err(|e| format!("h3 recv_response error: {e}"))?;
    let mut rsp = HttpDnsResponse::new(response)?;

    while let Some(partial_bytes) = send_stream
        .recv_data()
        .await
        .map_err(|e| format!("h3 recv_data error: {e}"))?
    {
        rsp.push_body(partial_bytes);
        if rsp.body_end() {
            break;
        }
    }

    rsp.into_dns_response()
}
