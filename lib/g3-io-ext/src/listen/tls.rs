/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::net::{self, SocketAddr};

use tokio::net::TcpStream;
use tokio_rustls::{TlsAcceptor, server::TlsStream};

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
