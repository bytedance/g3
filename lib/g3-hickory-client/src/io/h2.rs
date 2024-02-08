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
use std::io;
use std::net::SocketAddr;
use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use bytes::{Buf, Bytes, BytesMut};
use futures_util::future::{FutureExt, TryFutureExt};
use futures_util::{ready, Stream};
use h2::client::{Connection, SendRequest};
use hickory_proto::error::ProtoError;
use hickory_proto::iocompat::AsyncIoStdAsTokio;
use hickory_proto::op::Message;
use hickory_proto::tcp::{Connect, DnsTcpStream};
use hickory_proto::xfer::{DnsRequest, DnsRequestSender, DnsResponse, DnsResponseStream};
use http::{Response, Version};
use log::{debug, warn};

use super::http::request::HttpDnsRequestBuilder;
use crate::connect::tls::TlsConnect;
use crate::io::http::response::HttpDnsResponse;

pub fn connect_with_bind_addr<S: Connect, TC: TlsConnect<S> + Send + 'static>(
    name_server: SocketAddr,
    bind_addr: Option<SocketAddr>,
    tls_connector: TC,
) -> HttpsClientConnect<S, TC>
where
    TC::TlsStream: DnsTcpStream,
{
    let connect = S::connect_with_bind(name_server, bind_addr);

    HttpsClientConnect::<S, TC>(HttpsClientConnectState::TcpConnecting {
        connect,
        name_server,
        tls: Some(tls_connector),
    })
}

pub struct HttpsClientConnect<S, TC>(HttpsClientConnectState<S, TC>)
where
    S: Connect,
    TC: TlsConnect<S>;

impl<S, TC> Future for HttpsClientConnect<S, TC>
where
    S: Connect,
    TC: TlsConnect<S> + Unpin,
{
    type Output = Result<HttpsClientStream, ProtoError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.0.poll_unpin(cx)
    }
}

enum HttpsClientConnectState<S, TC>
where
    S: Connect,
    TC: TlsConnect<S>,
{
    TcpConnecting {
        connect: Pin<Box<dyn Future<Output = io::Result<S>> + Send>>,
        name_server: SocketAddr,
        tls: Option<TC>,
    },
    TlsConnecting {
        // TODO: also abstract away Tokio TLS in RuntimeProvider.
        tls: Pin<Box<dyn Future<Output = io::Result<TC::TlsStream>> + Send>>,
        name_server_name: Arc<str>,
        name_server: SocketAddr,
    },
    H2Handshake {
        handshake: Pin<
            Box<
                dyn Future<
                        Output = Result<
                            (
                                SendRequest<Bytes>,
                                Connection<AsyncIoStdAsTokio<TC::TlsStream>, Bytes>,
                            ),
                            h2::Error,
                        >,
                    > + Send,
            >,
        >,
        name_server_name: Arc<str>,
        name_server: SocketAddr,
    },
    Connected(Option<HttpsClientStream>),
}

impl<S, TC> Future for HttpsClientConnectState<S, TC>
where
    S: Connect,
    TC: TlsConnect<S> + Unpin,
{
    type Output = Result<HttpsClientStream, ProtoError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            let next = match *self {
                Self::TcpConnecting {
                    ref mut connect,
                    name_server,
                    ref mut tls,
                } => {
                    let tcp = ready!(connect.poll_unpin(cx))?;

                    debug!("tcp connection established to: {}", name_server);
                    let tls = tls
                        .take()
                        .expect("programming error, tls should not be None here");
                    let name_server_name = Arc::from(tls.server_name());

                    let tls_connect = tls.tls_connect(tcp);
                    Self::TlsConnecting {
                        name_server_name,
                        name_server,
                        tls: tls_connect,
                    }
                }
                Self::TlsConnecting {
                    ref name_server_name,
                    name_server,
                    ref mut tls,
                } => {
                    let tls = ready!(tls.poll_unpin(cx))?;
                    debug!("tls connection established to: {}", name_server);
                    let mut handshake = h2::client::Builder::new();
                    handshake.enable_push(false);

                    let handshake = handshake.handshake(AsyncIoStdAsTokio(tls));
                    Self::H2Handshake {
                        name_server_name: Arc::clone(name_server_name),
                        name_server,
                        handshake: Box::pin(handshake),
                    }
                }
                Self::H2Handshake {
                    ref name_server_name,
                    name_server,
                    ref mut handshake,
                } => {
                    let (send_request, connection) = ready!(handshake
                        .poll_unpin(cx)
                        .map_err(|e| ProtoError::from(format!("h2 handshake error: {e}"))))?;

                    // TODO: hand this back for others to run rather than spawning here?
                    debug!("h2 connection established to: {}", name_server);
                    tokio::spawn(
                        connection
                            .map_err(|e| warn!("h2 connection failed: {e}"))
                            .map(|_: Result<(), ()>| ()),
                    );

                    let client_stream =
                        HttpsClientStream::new(Arc::clone(name_server_name), send_request)?;
                    Self::Connected(Some(client_stream))
                }
                Self::Connected(ref mut conn) => {
                    return Poll::Ready(Ok(conn.take().expect("cannot poll after complete")))
                }
            };

            *self.as_mut().deref_mut() = next;
        }
    }
}

/// A DNS client connection for DNS-over-HTTPS
#[derive(Clone)]
#[must_use = "futures do nothing unless polled"]
pub struct HttpsClientStream {
    request_builder: Arc<HttpDnsRequestBuilder>,
    h2: SendRequest<Bytes>,
    is_shutdown: bool,
}

impl HttpsClientStream {
    pub fn new(name_server_name: Arc<str>, h2: SendRequest<Bytes>) -> Result<Self, ProtoError> {
        let request_builder =
            HttpDnsRequestBuilder::new(Version::HTTP_2, name_server_name.as_ref())?;
        Ok(HttpsClientStream {
            request_builder: Arc::new(request_builder),
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

        Box::pin(h2_send_recv(
            self.h2.clone(),
            Bytes::from(bytes),
            Arc::clone(&self.request_builder),
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

async fn h2_send_recv(
    h2: SendRequest<Bytes>,
    message: Bytes,
    request_builder: Arc<HttpDnsRequestBuilder>,
) -> Result<DnsResponse, ProtoError> {
    let mut h2 = match h2.ready().await {
        Ok(h2) => h2,
        Err(err) => {
            // TODO: make specific error
            return Err(ProtoError::from(format!("h2 send_request error: {err}")));
        }
    };

    // build up the http request
    let request = request_builder.post(message.remaining());

    // Send the request
    let (response_future, mut send_stream) = h2
        .send_request(request, false)
        .map_err(|err| ProtoError::from(format!("h2 send_request error: {err}")))?;
    send_stream
        .send_data(message, true)
        .map_err(|e| ProtoError::from(format!("h2 send_data error: {e}")))?;

    let response_stream = response_future
        .await
        .map_err(|err| ProtoError::from(format!("received a stream error: {err}")))?;
    let (parts, mut recv_stream) = response_stream.into_parts();

    let rsp = HttpDnsResponse::new(Response::from_parts(parts, ()))?;

    // get the length of packet
    let content_length = rsp.content_length();

    // TODO: what is a good max here?
    // clamp(512, 4096) says make sure it is at least 512 bytes, and min 4096 says it is at most 4k
    // just a little protection from malicious actors.
    let mut response_bytes =
        BytesMut::with_capacity(content_length.unwrap_or(512).clamp(512, 4096));

    while let Some(partial_bytes) = recv_stream.data().await {
        let partial_bytes =
            partial_bytes.map_err(|e| ProtoError::from(format!("bad http request: {e}")))?;

        debug!("got bytes: {}", partial_bytes.len());
        response_bytes.extend(partial_bytes);

        // assert the length
        if let Some(content_length) = content_length {
            if response_bytes.len() >= content_length {
                break;
            }
        }
    }

    // assert the length
    if let Some(content_length) = content_length {
        if response_bytes.len() != content_length {
            // TODO: make explicit error type
            return Err(ProtoError::from(format!(
                "expected byte length: {}, got: {}",
                content_length,
                response_bytes.len()
            )));
        }
    }

    // Was it a successful request?
    if !rsp.status().is_success() {
        let error_string = String::from_utf8_lossy(response_bytes.as_ref());

        // TODO: make explicit error type
        return Err(ProtoError::from(format!(
            "http unsuccessful code: {}, message: {}",
            rsp.status(),
            error_string
        )));
    }

    // and finally convert the bytes into a DNS message
    let message = Message::from_vec(&response_bytes)?;
    Ok(DnsResponse::new(message, response_bytes.to_vec()))
}
