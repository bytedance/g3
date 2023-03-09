/*
 * Copyright 2023 ByteDance and/or its affiliates.
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

use std::io;
use std::net::{self, SocketAddr};

use tokio::net::TcpStream;
use tokio_rustls::{server::TlsStream, TlsAcceptor};

use super::LimitedTcpListener;

pub struct LimitedTlsListener {
    tcp_listener: LimitedTcpListener,
    tls_acceptor: TlsAcceptor,
}

impl LimitedTlsListener {
    pub fn from_std(listener: net::TcpListener, tls_acceptor: TlsAcceptor) -> io::Result<Self> {
        let tcp_listener = LimitedTcpListener::from_std(listener)?;
        Ok(LimitedTlsListener {
            tcp_listener,
            tls_acceptor,
        })
    }

    pub fn set_offline(&mut self) -> bool {
        self.tcp_listener.set_offline()
    }

    pub async fn accept(
        &mut self,
    ) -> io::Result<Option<(TlsStream<TcpStream>, SocketAddr, SocketAddr)>> {
        if let Some((stream, peer_addr, local_addr)) = self.tcp_listener.accept().await? {
            let tls_stream = self.tls_acceptor.accept(stream).await?;
            Ok(Some((tls_stream, peer_addr, local_addr)))
        } else {
            Ok(None)
        }
    }
}
