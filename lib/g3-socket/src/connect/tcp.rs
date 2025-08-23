/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::net::SocketAddr;

use tokio::net::TcpStream;

use g3_types::net::{TcpKeepAliveConfig, TcpMiscSockOpts};

use crate::BindAddr;

pub struct TcpConnectInfo {
    pub server: SocketAddr,
    pub bind: BindAddr,
    pub keepalive: TcpKeepAliveConfig,
    pub misc_opts: TcpMiscSockOpts,
}

impl TcpConnectInfo {
    pub async fn tcp_connect(&self) -> io::Result<TcpStream> {
        let socket = crate::tcp::new_socket_to(
            self.server.ip(),
            &self.bind,
            &self.keepalive,
            &self.misc_opts,
            true,
        )?;
        let tcp_stream = socket.connect(self.server).await?;
        Ok(tcp_stream)
    }
}
