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

use bytes::Bytes;
use futures_util::future::{FutureExt, TryFutureExt};
use futures_util::ready;
use h2::client::{Connection, SendRequest};
use hickory_proto::error::ProtoError;
use hickory_proto::h2::HttpsClientStream;
use hickory_proto::iocompat::AsyncIoStdAsTokio;
use hickory_proto::tcp::{Connect, DnsTcpStream};
use log::{debug, warn};

use crate::connect::tls::TlsConnect;

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
    Errored(Option<ProtoError>),
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
                        tls: Box::pin(tls_connect),
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

                    Self::Connected(Some(HttpsClientStream {
                        name_server_name: Arc::clone(name_server_name),
                        name_server,
                        h2: send_request,
                        is_shutdown: false,
                    }))
                }
                Self::Connected(ref mut conn) => {
                    return Poll::Ready(Ok(conn.take().expect("cannot poll after complete")))
                }
                Self::Errored(ref mut err) => {
                    return Poll::Ready(Err(err.take().expect("cannot poll after complete")))
                }
            };

            *self.as_mut().deref_mut() = next;
        }
    }
}
