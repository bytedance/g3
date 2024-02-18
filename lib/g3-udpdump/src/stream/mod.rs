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

use std::io;
use std::net::SocketAddr;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::UdpSocket;
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use crate::ExportedPduDissectorHint;

mod config;
pub use config::StreamDumpConfig;

mod sink;
use sink::Sinker;

mod header;
use header::PduHeader;
pub use header::{ToClientPduHeader, ToRemotePduHeader};

mod state;
use state::StreamDumpState;

mod write;
pub use write::{StreamDumpWriter, ToClientStreamDumpWriter, ToRemoteStreamDumpWriter};

mod read;
pub use read::{FromClientStreamDumpReader, FromRemoteStreamDumpReader, StreamDumpReader};

pub struct StreamDumper {
    config: StreamDumpConfig,
    sender: mpsc::UnboundedSender<Vec<u8>>,
}

impl StreamDumper {
    pub fn new(config: StreamDumpConfig, runtime: &Handle) -> io::Result<Self> {
        let socket =
            g3_socket::udp::new_std_socket_to(config.peer, None, config.buffer, config.opts)?;
        socket.connect(config.peer)?;

        let (sender, receiver) = mpsc::unbounded_channel();

        runtime.spawn(async move {
            let socket = UdpSocket::from_std(socket).unwrap();
            Sinker::new(receiver, socket).into_running().await;
        });

        Ok(StreamDumper { config, sender })
    }

    pub fn wrap_writer<CW, RW>(
        &self,
        client_addr: SocketAddr,
        remote_addr: SocketAddr,
        dissector_hint: ExportedPduDissectorHint,
        client_writer: CW,
        remote_writer: RW,
    ) -> (ToClientStreamDumpWriter<CW>, ToRemoteStreamDumpWriter<RW>)
    where
        CW: AsyncWrite,
        RW: AsyncWrite,
    {
        let (to_c, to_r) = header::new_pair(client_addr, remote_addr, dissector_hint);
        let cw = StreamDumpWriter::new(
            client_writer,
            to_c,
            self.sender.clone(),
            self.config.packet_size,
        );
        let rw = StreamDumpWriter::new(
            remote_writer,
            to_r,
            self.sender.clone(),
            self.config.packet_size,
        );
        (cw, rw)
    }

    pub fn wrap_remote_io<R, W>(
        &self,
        client_addr: SocketAddr,
        remote_addr: SocketAddr,
        dissector_hint: ExportedPduDissectorHint,
        remote_reader: R,
        remote_writer: W,
    ) -> (FromRemoteStreamDumpReader<R>, ToRemoteStreamDumpWriter<W>)
    where
        R: AsyncRead,
        W: AsyncWrite,
    {
        let (to_c, to_r) = header::new_pair(client_addr, remote_addr, dissector_hint);
        let r = StreamDumpReader::new(
            remote_reader,
            to_c,
            self.sender.clone(),
            self.config.packet_size,
        );
        let w = StreamDumpWriter::new(
            remote_writer,
            to_r,
            self.sender.clone(),
            self.config.packet_size,
        );
        (r, w)
    }

    pub fn wrap_client_io<R, W>(
        &self,
        client_addr: SocketAddr,
        remote_addr: SocketAddr,
        dissector_hint: ExportedPduDissectorHint,
        client_reader: R,
        client_writer: W,
    ) -> (FromClientStreamDumpReader<R>, ToClientStreamDumpWriter<W>)
    where
        R: AsyncRead,
        W: AsyncWrite,
    {
        let (to_c, to_r) = header::new_pair(client_addr, remote_addr, dissector_hint);
        let r = StreamDumpReader::new(
            client_reader,
            to_r,
            self.sender.clone(),
            self.config.packet_size,
        );
        let w = StreamDumpWriter::new(
            client_writer,
            to_c,
            self.sender.clone(),
            self.config.packet_size,
        );
        (r, w)
    }
}
