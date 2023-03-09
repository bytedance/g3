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
use std::net::{IpAddr, SocketAddr};

use async_trait::async_trait;
use tokio::net::TcpStream;

use g3_ftp_client::FtpConnectionProvider;
use g3_types::net::UpstreamAddr;

#[derive(Default)]
pub(crate) struct LocalConnectionProvider {
    bind_ip: Option<IpAddr>,
    remote_addr: Option<SocketAddr>,
}

impl LocalConnectionProvider {
    pub(crate) fn set_bind_ip(&mut self, ip: IpAddr) {
        self.bind_ip = Some(ip);
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
                self.bind_ip,
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
                    self.bind_ip,
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
