/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::net::{IpAddr, SocketAddr};

use async_trait::async_trait;
use tokio::net::TcpStream;

use g3_ftp_client::FtpConnectionProvider;
use g3_socket::BindAddr;
use g3_types::net::UpstreamAddr;

#[derive(Default)]
pub(crate) struct LocalConnectionProvider {
    bind: BindAddr,
    remote_addr: Option<SocketAddr>,
}

impl LocalConnectionProvider {
    pub(crate) fn set_bind_ip(&mut self, ip: IpAddr) {
        self.bind = BindAddr::Ip(ip);
    }
}

#[async_trait]
impl FtpConnectionProvider<TcpStream, io::Error, ()> for LocalConnectionProvider {
    async fn new_control_connection(
        &mut self,
        upstream: &UpstreamAddr,
        _user_data: &(),
    ) -> io::Result<TcpStream> {
        let mut err = io::Error::new(io::ErrorKind::AddrNotAvailable, "no addr resolved");
        for addr in tokio::net::lookup_host(upstream.to_string()).await? {
            let socket = g3_socket::tcp::new_socket_to(
                addr.ip(),
                &self.bind,
                &Default::default(),
                &Default::default(),
                true,
            )?;
            match socket.connect(addr).await {
                Ok(stream) => {
                    self.remote_addr = Some(addr);
                    return Ok(stream);
                }
                Err(e) => err = e,
            }
        }

        Err(err)
    }

    async fn new_data_connection(
        &mut self,
        server: &UpstreamAddr,
        _user_data: &(),
    ) -> io::Result<TcpStream> {
        match self.remote_addr {
            Some(addr) => {
                let data_addr = SocketAddr::new(addr.ip(), server.port());
                let socket = g3_socket::tcp::new_socket_to(
                    data_addr.ip(),
                    &self.bind,
                    &Default::default(),
                    &Default::default(),
                    false,
                )?;
                socket.connect(data_addr).await
            }
            None => Err(io::Error::new(
                io::ErrorKind::AddrNotAvailable,
                "no resolved upstream addr found",
            )),
        }
    }
}
