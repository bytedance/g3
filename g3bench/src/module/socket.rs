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

#[cfg(feature = "quic")]
use std::net::UdpSocket;
use std::net::{IpAddr, SocketAddr};

use anyhow::anyhow;
use clap::{Arg, ArgMatches, Command, value_parser};
use tokio::net::TcpStream;

use g3_socket::{BindAddr, TcpConnectInfo, UdpConnectInfo};
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "illumos",
    target_os = "solaris"
))]
use g3_types::net::Interface;

const SOCKET_ARG_LOCAL_ADDRESS: &str = "local-address";
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "illumos",
    target_os = "solaris"
))]
const SOCKET_ARG_INTERFACE: &str = "interface";

pub(crate) trait AppendSocketArgs {
    fn append_socket_args(self) -> Self;
}

#[derive(Default)]
pub(crate) struct SocketArgs {
    bind: BindAddr,
}

impl SocketArgs {
    pub(crate) async fn tcp_connect_to(&self, peer: SocketAddr) -> anyhow::Result<TcpStream> {
        let socket = g3_socket::tcp::new_socket_to(
            peer.ip(),
            &self.bind,
            &Default::default(),
            &Default::default(),
            true,
        )
        .map_err(|e| anyhow!("failed to setup socket to {peer}: {e:?}"))?;
        socket
            .connect(peer)
            .await
            .map_err(|e| anyhow!("connect to {peer} error: {e:?}"))
    }

    #[cfg(feature = "quic")]
    pub(crate) fn udp_std_socket_to(&self, peer: SocketAddr) -> anyhow::Result<UdpSocket> {
        g3_socket::udp::new_std_socket_to(peer, &self.bind, Default::default(), Default::default())
            .map_err(|e| anyhow!("failed to setup local udp socket: {e}"))
    }

    pub(crate) fn hickory_udp_connect_info(&self, server: SocketAddr) -> UdpConnectInfo {
        UdpConnectInfo {
            server,
            bind: self.bind,
            buf_conf: Default::default(),
            misc_opts: Default::default(),
        }
    }

    pub(crate) fn hickory_tcp_connect_info(&self, server: SocketAddr) -> TcpConnectInfo {
        TcpConnectInfo {
            server,
            bind: self.bind,
            keepalive: Default::default(),
            misc_opts: Default::default(),
        }
    }

    pub(crate) fn parse_args(&mut self, args: &ArgMatches) -> anyhow::Result<()> {
        if let Some(ip) = args.get_one::<IpAddr>(SOCKET_ARG_LOCAL_ADDRESS) {
            self.bind = BindAddr::Ip(*ip);
        }
        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "illumos",
            target_os = "solaris"
        ))]
        if let Some(name) = args.get_one::<Interface>(SOCKET_ARG_INTERFACE) {
            self.bind = BindAddr::Interface(*name);
        }
        Ok(())
    }
}

impl AppendSocketArgs for Command {
    fn append_socket_args(self) -> Self {
        append_socket_args(self)
    }
}

pub(crate) fn append_socket_args(mut cmd: Command) -> Command {
    macro_rules! add_arg {
        ($arg:expr) => {
            cmd = cmd.arg($arg);
        };
    }

    add_arg!(
        Arg::new(SOCKET_ARG_LOCAL_ADDRESS)
            .value_name("LOCAL IP ADDRESS")
            .short('B')
            .long(SOCKET_ARG_LOCAL_ADDRESS)
            .num_args(1)
            .value_parser(value_parser!(IpAddr))
    );
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "illumos",
        target_os = "solaris"
    ))]
    add_arg!(
        Arg::new(SOCKET_ARG_INTERFACE)
            .value_name("INTERFACE NAME/INDEX")
            .short('I')
            .long(SOCKET_ARG_INTERFACE)
            .num_args(1)
            .value_parser(value_parser!(Interface))
            .conflicts_with(SOCKET_ARG_LOCAL_ADDRESS)
    );
    cmd
}
