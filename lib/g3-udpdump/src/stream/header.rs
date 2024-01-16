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

use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use g3_dpi::Protocol;

pub(super) fn new_pair(
    client: SocketAddr,
    remote: SocketAddr,
    protocol: Protocol,
) -> (ToClientPduHeader, ToRemotePduHeader) {
    let stats = Arc::new(TcpDissectorStats::default());
    let to_client = ToClientPduHeader::new(client, remote, protocol, stats.clone());
    let to_remote = ToRemotePduHeader::new(client, remote, protocol, stats);
    (to_client, to_remote)
}

pub(super) struct TcpDissectorStats {
    write_to_client: AtomicU32,
    write_to_remote: AtomicU32,
}

impl Default for TcpDissectorStats {
    fn default() -> Self {
        TcpDissectorStats {
            write_to_client: AtomicU32::new(1),
            write_to_remote: AtomicU32::new(1),
        }
    }
}

impl TcpDissectorStats {
    fn write_to_client(&self) -> u32 {
        self.write_to_client.load(Ordering::Relaxed)
    }

    fn add_write_to_client(&self, size: usize) {
        self.write_to_client
            .fetch_add(size as u32, Ordering::Relaxed);
    }

    fn write_to_remote(&self) -> u32 {
        self.write_to_remote.load(Ordering::Relaxed)
    }

    fn add_write_to_remote(&self, size: usize) {
        self.write_to_remote
            .fetch_add(size as u32, Ordering::Relaxed);
    }
}

pub trait PduHeader {
    fn new_header(&mut self, pkt_size: usize) -> Vec<u8>;
    fn update_tcp_dissector_data(&self, hdr: &mut Vec<u8>, data_len: usize);
    fn record_written_data(&self, data_len: usize);
}

pub struct ToClientPduHeader {
    client: SocketAddr,
    remote: SocketAddr,
    protocol: Protocol,
    tcp_dissector_stats: Arc<TcpDissectorStats>,
    tcp_dissector_offset: usize,
    total_len: usize,
}

impl ToClientPduHeader {
    fn new(
        client: SocketAddr,
        remote: SocketAddr,
        protocol: Protocol,
        tcp_dissector_stats: Arc<TcpDissectorStats>,
    ) -> Self {
        ToClientPduHeader {
            client,
            remote,
            protocol,
            tcp_dissector_stats,
            tcp_dissector_offset: 0,
            total_len: 0,
        }
    }
}

impl PduHeader for ToClientPduHeader {
    fn new_header(&mut self, pkt_size: usize) -> Vec<u8> {
        let mut hdr = new_fixed_header(pkt_size, self.remote, self.client, self.protocol);
        self.tcp_dissector_offset = hdr.len();
        push_var_header(&mut hdr);
        self.total_len = hdr.len();
        hdr
    }

    fn update_tcp_dissector_data(&self, hdr: &mut Vec<u8>, data_len: usize) {
        let mut offset = self.tcp_dissector_offset + 6;
        debug_assert!(offset + 12 < hdr.len());

        let seq = self.tcp_dissector_stats.write_to_client();
        hdr[offset..offset + 4].copy_from_slice(&seq.to_be_bytes());
        offset += 4;

        let next_seq = ((seq as usize + data_len) & 0xFFFFFFFF) as u32;
        hdr[offset..offset + 4].copy_from_slice(&next_seq.to_be_bytes());
        offset += 4;

        let last_ack = self.tcp_dissector_stats.write_to_remote();
        hdr[offset..offset + 4].copy_from_slice(&last_ack.to_be_bytes());
    }

    fn record_written_data(&self, data_len: usize) {
        self.tcp_dissector_stats.add_write_to_client(data_len);
    }
}

pub struct ToRemotePduHeader {
    client: SocketAddr,
    remote: SocketAddr,
    protocol: Protocol,
    tcp_dissector_stats: Arc<TcpDissectorStats>,
    tcp_dissector_offset: usize,
    total_len: usize,
}

impl ToRemotePduHeader {
    fn new(
        client: SocketAddr,
        remote: SocketAddr,
        protocol: Protocol,
        tcp_dissector_stats: Arc<TcpDissectorStats>,
    ) -> Self {
        ToRemotePduHeader {
            client,
            remote,
            protocol,
            tcp_dissector_stats,
            tcp_dissector_offset: 0,
            total_len: 0,
        }
    }
}

impl PduHeader for ToRemotePduHeader {
    fn new_header(&mut self, pkt_size: usize) -> Vec<u8> {
        let mut hdr = new_fixed_header(pkt_size, self.client, self.remote, self.protocol);
        self.tcp_dissector_offset = hdr.len();
        push_var_header(&mut hdr);
        self.total_len = hdr.len();
        hdr
    }

    fn update_tcp_dissector_data(&self, hdr: &mut Vec<u8>, data_len: usize) {
        let mut offset = self.tcp_dissector_offset + 6;
        debug_assert!(offset + 12 < hdr.len());

        let seq = self.tcp_dissector_stats.write_to_remote();
        hdr[offset..offset + 4].copy_from_slice(&seq.to_be_bytes());
        offset += 4;

        let next_seq = ((seq as usize + data_len) & 0xFFFFFFFF) as u32;
        hdr[offset..offset + 4].copy_from_slice(&next_seq.to_be_bytes());
        offset += 4;

        let last_ack = self.tcp_dissector_stats.write_to_client();
        hdr[offset..offset + 4].copy_from_slice(&last_ack.to_be_bytes());
    }

    fn record_written_data(&self, data_len: usize) {
        self.tcp_dissector_stats.add_write_to_remote(data_len);
    }
}

fn new_fixed_header(
    pkt_size: usize,
    src_addr: SocketAddr,
    dst_addr: SocketAddr,
    protocol: Protocol,
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(pkt_size);

    let dissector = protocol.wireshark_dissector();
    if !dissector.is_empty() {
        buf.extend_from_slice(&[0x00, 0x0c]);
        let len = (dissector.len() & 0xFFFF) as u16;
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(dissector.as_bytes());
    }

    // src ip
    match src_addr.ip() {
        IpAddr::V4(ip4) => {
            buf.extend_from_slice(&[0x00, 0x14, 0x00, 0x04]); // ipv4 src ip
            buf.extend_from_slice(&ip4.octets());
        }
        IpAddr::V6(ip6) => {
            buf.extend_from_slice(&[0x00, 0x16, 0x00, 0x10]); // ipv6 src ip
            buf.extend_from_slice(&ip6.octets());
        }
    }

    // dst ip
    match dst_addr.ip() {
        IpAddr::V4(ip4) => {
            buf.extend_from_slice(&[0x00, 0x15, 0x00, 0x04]); // ipv4 dst ip
            buf.extend_from_slice(&ip4.octets());
        }
        IpAddr::V6(ip6) => {
            buf.extend_from_slice(&[0x00, 0x17, 0x00, 0x10]); // ipv6 dst ip
            buf.extend_from_slice(&ip6.octets());
        }
    }

    // port type = tcp
    buf.extend_from_slice(&[0x00, 0x18, 0x00, 0x04, 0x00, 0x00, 0x00, 0x02]);

    // src port
    let src_port = src_addr.port().to_be_bytes();
    buf.extend_from_slice(&[0x00, 0x19, 0x00, 0x04, 0x00, 0x00, src_port[0], src_port[1]]);

    // dst port
    let dst_port = dst_addr.port().to_be_bytes();
    buf.extend_from_slice(&[0x00, 0x1a, 0x00, 0x04, 0x00, 0x00, dst_port[0], dst_port[1]]);

    buf
}

fn push_var_header(buf: &mut Vec<u8>) {
    // tcp dissector data
    buf.extend_from_slice(&[0x00, 0x22, 0x00, 0x13, 0x00, 0x01]); // version 1
    buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // seq
    buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // next seq
    buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // last ack
    buf.extend_from_slice(&[0x01, 0x00, 0x18, 0x00, 0x00]); // reassembled, flags 0x0018

    // end of option
    buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
}
