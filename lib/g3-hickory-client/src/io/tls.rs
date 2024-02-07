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
use std::pin::Pin;

use futures_util::future::TryFutureExt;
use hickory_proto::error::ProtoError;
use hickory_proto::tcp::{Connect, DnsTcpStream, TcpClientStream, TcpStream};
use hickory_proto::xfer::StreamReceiver;
use hickory_proto::BufDnsStreamHandle;

use crate::connect::tls::TlsConnect;

pub fn connect_with_bind_addr<S: Connect, TC: TlsConnect<S> + Send + 'static>(
    name_server: SocketAddr,
    bind_addr: Option<SocketAddr>,
    tls_connector: TC,
) -> (
    Pin<
        Box<dyn Future<Output = Result<TcpClientStream<TC::TlsStream>, ProtoError>> + Send + Unpin>,
    >,
    BufDnsStreamHandle,
)
where
    TC::TlsStream: DnsTcpStream,
{
    let (stream_future, sender) = tls_connect_with_bind_addr(name_server, bind_addr, tls_connector);

    let new_future = Box::pin(
        stream_future
            .map_ok(TcpClientStream::from_stream)
            .map_err(ProtoError::from),
    );

    (new_future, sender)
}

pub fn tls_connect_with_bind_addr<S: Connect, TC: TlsConnect<S> + Send + 'static>(
    name_server: SocketAddr,
    bind_addr: Option<SocketAddr>,
    tls_connector: TC,
) -> (
    Pin<Box<dyn Future<Output = Result<TcpStream<TC::TlsStream>, io::Error>> + Send>>,
    BufDnsStreamHandle,
)
where
    TC::TlsStream: DnsTcpStream,
{
    let (message_sender, outbound_messages) = BufDnsStreamHandle::new(name_server);

    // This set of futures collapses the next tcp socket into a stream which can be used for
    //  sending and receiving tcp packets.
    let stream = Box::pin(connect_tls(
        name_server,
        bind_addr,
        tls_connector,
        outbound_messages,
    ));

    (stream, message_sender)
}

async fn connect_tls<S: Connect, TC: TlsConnect<S> + Send + 'static>(
    name_server: SocketAddr,
    bind_addr: Option<SocketAddr>,
    tls_connector: TC,
    outbound_messages: StreamReceiver,
) -> io::Result<TcpStream<TC::TlsStream>>
where
    TC::TlsStream: DnsTcpStream,
{
    let stream = S::connect_with_bind(name_server, bind_addr).await?;
    let tls_stream = tls_connector.tls_connect(stream).await?;

    Ok(TcpStream::from_stream_with_receiver(
        tls_stream,
        name_server,
        outbound_messages,
    ))
}
