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

use crate::ExportedPduDissectorHint;

pub(super) fn new_pair(
    client: SocketAddr,
    remote: SocketAddr,
    dissector_hint: ExportedPduDissectorHint,
) -> (ToClientPduHeader, ToRemotePduHeader) {
    let state = Arc::new(TcpDissectorState::new(dissector_hint));
    let to_client = ToClientPduHeader::new(client, remote, state.clone());
    let to_remote = ToRemotePduHeader::new(client, remote, state);
    (to_client, to_remote)
}

#[derive(Clone, Copy)]
pub struct StreamDumpProxyAddresses {
    pub client: SocketAddr,
    pub local_server: SocketAddr,
    pub local_client: SocketAddr,
    pub remote: SocketAddr,
}

pub(super) fn new_proxy_pair(
    addresses: StreamDumpProxyAddresses,
    dissector_hint: ExportedPduDissectorHint,
) -> (ProxyToClientPduHeader, ProxyToRemotePduHeader) {
    let state = Arc::new(TcpDissectorState::new(dissector_hint));
    let to_client = ProxyToClientPduHeader::new(addresses, state.clone());
    let to_remote = ProxyToRemotePduHeader::new(addresses, state);
    (to_client, to_remote)
}

pub(super) struct TcpDissectorState {
    dissector_hint: ExportedPduDissectorHint,
    write_to_client: AtomicU32,
    write_to_remote: AtomicU32,
}

impl TcpDissectorState {
    fn new(dissector_hint: ExportedPduDissectorHint) -> Self {
        TcpDissectorState {
            dissector_hint,
            write_to_client: AtomicU32::new(1),
            write_to_remote: AtomicU32::new(1),
        }
    }

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

fn to_client_update_tcp_dissector_data(
    tcp_dissector_state: &Arc<TcpDissectorState>,
    hdr: &mut [u8],
    tcp_dissector_offset: usize,
    data_len: usize,
) {
    let mut offset = tcp_dissector_offset + 6;
    debug_assert!(offset + 12 < hdr.len());

    let seq = tcp_dissector_state.write_to_client();
    hdr[offset..offset + 4].copy_from_slice(&seq.to_be_bytes());
    offset += 4;

    let next_seq = seq.wrapping_add(data_len as _);
    hdr[offset..offset + 4].copy_from_slice(&next_seq.to_be_bytes());
    offset += 4;

    let last_ack = tcp_dissector_state.write_to_remote();
    hdr[offset..offset + 4].copy_from_slice(&last_ack.to_be_bytes());
}

pub struct ToClientPduHeader {
    client: SocketAddr,
    remote: SocketAddr,
    tcp_dissector_state: Arc<TcpDissectorState>,
    tcp_dissector_offset: usize,
    total_len: usize,
}

impl ToClientPduHeader {
    fn new(
        client: SocketAddr,
        remote: SocketAddr,
        tcp_dissector_state: Arc<TcpDissectorState>,
    ) -> Self {
        ToClientPduHeader {
            client,
            remote,
            tcp_dissector_state,
            tcp_dissector_offset: 0,
            total_len: 0,
        }
    }
}

impl PduHeader for ToClientPduHeader {
    fn new_header(&mut self, pkt_size: usize) -> Vec<u8> {
        let mut hdr = Vec::with_capacity(pkt_size);
        push_fixed_tcp_header(
            &mut hdr,
            self.remote,
            self.client,
            &self.tcp_dissector_state.dissector_hint,
        );
        self.tcp_dissector_offset = hdr.len();
        push_var_tcp_header(&mut hdr);
        self.total_len = hdr.len();
        hdr
    }

    fn update_tcp_dissector_data(&self, hdr: &mut Vec<u8>, data_len: usize) {
        to_client_update_tcp_dissector_data(
            &self.tcp_dissector_state,
            hdr,
            self.tcp_dissector_offset,
            data_len,
        );
    }

    fn record_written_data(&self, data_len: usize) {
        self.tcp_dissector_state.add_write_to_client(data_len);
    }
}

pub struct ProxyToClientPduHeader {
    addr: StreamDumpProxyAddresses,
    tcp_dissector_state: Arc<TcpDissectorState>,
    tcp_dissector_offset: usize,
    total_len: usize,
}

impl ProxyToClientPduHeader {
    fn new(addr: StreamDumpProxyAddresses, tcp_dissector_state: Arc<TcpDissectorState>) -> Self {
        ProxyToClientPduHeader {
            addr,
            tcp_dissector_state,
            tcp_dissector_offset: 0,
            total_len: 0,
        }
    }
}

impl PduHeader for ProxyToClientPduHeader {
    fn new_header(&mut self, pkt_size: usize) -> Vec<u8> {
        let mut hdr = Vec::with_capacity(pkt_size);
        push_proxy_inner_header(&mut hdr, self.addr.local_server, self.addr.client);
        push_fixed_tcp_header(
            &mut hdr,
            self.addr.remote,
            self.addr.local_client,
            &self.tcp_dissector_state.dissector_hint,
        );
        self.tcp_dissector_offset = hdr.len();
        push_var_tcp_header(&mut hdr);
        self.total_len = hdr.len();
        hdr
    }

    fn update_tcp_dissector_data(&self, hdr: &mut Vec<u8>, data_len: usize) {
        to_client_update_tcp_dissector_data(
            &self.tcp_dissector_state,
            hdr,
            self.tcp_dissector_offset,
            data_len,
        );
    }

    fn record_written_data(&self, data_len: usize) {
        self.tcp_dissector_state.add_write_to_client(data_len);
    }
}

fn to_remote_update_tcp_dissector_data(
    tcp_dissector_state: &Arc<TcpDissectorState>,
    hdr: &mut [u8],
    tcp_dissector_offset: usize,
    data_len: usize,
) {
    let mut offset = tcp_dissector_offset + 6;
    debug_assert!(offset + 12 < hdr.len());

    let seq = tcp_dissector_state.write_to_remote();
    hdr[offset..offset + 4].copy_from_slice(&seq.to_be_bytes());
    offset += 4;

    let next_seq = seq.wrapping_add(data_len as _);
    hdr[offset..offset + 4].copy_from_slice(&next_seq.to_be_bytes());
    offset += 4;

    let last_ack = tcp_dissector_state.write_to_client();
    hdr[offset..offset + 4].copy_from_slice(&last_ack.to_be_bytes());
}

pub struct ToRemotePduHeader {
    client: SocketAddr,
    remote: SocketAddr,
    tcp_dissector_state: Arc<TcpDissectorState>,
    tcp_dissector_offset: usize,
    total_len: usize,
}

impl ToRemotePduHeader {
    fn new(
        client: SocketAddr,
        remote: SocketAddr,
        tcp_dissector_state: Arc<TcpDissectorState>,
    ) -> Self {
        ToRemotePduHeader {
            client,
            remote,
            tcp_dissector_state,
            tcp_dissector_offset: 0,
            total_len: 0,
        }
    }
}

impl PduHeader for ToRemotePduHeader {
    fn new_header(&mut self, pkt_size: usize) -> Vec<u8> {
        let mut hdr = Vec::with_capacity(pkt_size);
        push_fixed_tcp_header(
            &mut hdr,
            self.client,
            self.remote,
            &self.tcp_dissector_state.dissector_hint,
        );
        self.tcp_dissector_offset = hdr.len();
        push_var_tcp_header(&mut hdr);
        self.total_len = hdr.len();
        hdr
    }

    fn update_tcp_dissector_data(&self, hdr: &mut Vec<u8>, data_len: usize) {
        to_remote_update_tcp_dissector_data(
            &self.tcp_dissector_state,
            hdr,
            self.tcp_dissector_offset,
            data_len,
        );
    }

    fn record_written_data(&self, data_len: usize) {
        self.tcp_dissector_state.add_write_to_remote(data_len);
    }
}

pub struct ProxyToRemotePduHeader {
    addr: StreamDumpProxyAddresses,
    tcp_dissector_state: Arc<TcpDissectorState>,
    tcp_dissector_offset: usize,
    total_len: usize,
}

impl ProxyToRemotePduHeader {
    fn new(addr: StreamDumpProxyAddresses, tcp_dissector_state: Arc<TcpDissectorState>) -> Self {
        ProxyToRemotePduHeader {
            addr,
            tcp_dissector_state,
            tcp_dissector_offset: 0,
            total_len: 0,
        }
    }
}

impl PduHeader for ProxyToRemotePduHeader {
    fn new_header(&mut self, pkt_size: usize) -> Vec<u8> {
        let mut hdr = Vec::with_capacity(pkt_size);
        push_proxy_inner_header(&mut hdr, self.addr.client, self.addr.local_server);
        push_fixed_tcp_header(
            &mut hdr,
            self.addr.local_client,
            self.addr.remote,
            &self.tcp_dissector_state.dissector_hint,
        );
        self.tcp_dissector_offset = hdr.len();
        push_var_tcp_header(&mut hdr);
        self.total_len = hdr.len();
        hdr
    }

    fn update_tcp_dissector_data(&self, hdr: &mut Vec<u8>, data_len: usize) {
        to_remote_update_tcp_dissector_data(
            &self.tcp_dissector_state,
            hdr,
            self.tcp_dissector_offset,
            data_len,
        );
    }

    fn record_written_data(&self, data_len: usize) {
        self.tcp_dissector_state.add_write_to_remote(data_len);
    }
}

const EXP_PDU_TAG_IPV4_SRC: u8 = 20;
const EXP_PDU_TAG_IPV4_DST: u8 = 21;
const EXP_PDU_TAG_IPV6_SRC: u8 = 22;
const EXP_PDU_TAG_IPV6_DST: u8 = 23;

const EXP_PDU_TAG_PORT_TYPE: u8 = 24;
const EXP_PDU_TAG_SRC_PORT: u8 = 25;
const EXP_PDU_TAG_DST_PORT: u8 = 26;

const EXP_PDU_PT_TCP: u8 = 2;

const EXP_PDU_TAG_PORT_TYPE_LEN: u8 = 4;
const EXP_PDU_TAG_PORT_LEN: u8 = 4;

fn push_addr(hdr: &mut Vec<u8>, src_addr: SocketAddr, dst_addr: SocketAddr) {
    // src ip
    match src_addr.ip() {
        IpAddr::V4(ip4) => {
            hdr.extend_from_slice(&[0x00, EXP_PDU_TAG_IPV4_SRC, 0x00, 0x04]);
            hdr.extend_from_slice(&ip4.octets());
        }
        IpAddr::V6(ip6) => {
            hdr.extend_from_slice(&[0x00, EXP_PDU_TAG_IPV6_SRC, 0x00, 0x10]); // ipv6 src ip
            hdr.extend_from_slice(&ip6.octets());
        }
    }

    // dst ip
    match dst_addr.ip() {
        IpAddr::V4(ip4) => {
            hdr.extend_from_slice(&[0x00, EXP_PDU_TAG_IPV4_DST, 0x00, 0x04]);
            hdr.extend_from_slice(&ip4.octets());
        }
        IpAddr::V6(ip6) => {
            hdr.extend_from_slice(&[0x00, EXP_PDU_TAG_IPV6_DST, 0x00, 0x10]); // ipv6 dst ip
            hdr.extend_from_slice(&ip6.octets());
        }
    }

    // src port
    let src_port = src_addr.port().to_be_bytes();
    hdr.extend_from_slice(&[
        0x00,
        EXP_PDU_TAG_SRC_PORT,
        0x00,
        EXP_PDU_TAG_PORT_LEN,
        0x00,
        0x00,
        src_port[0],
        src_port[1],
    ]);
    // dst port
    let dst_port = dst_addr.port().to_be_bytes();
    hdr.extend_from_slice(&[
        0x00,
        EXP_PDU_TAG_DST_PORT,
        0x00,
        EXP_PDU_TAG_PORT_LEN,
        0x00,
        0x00,
        dst_port[0],
        dst_port[1],
    ]);
}

fn push_proxy_inner_header(hdr: &mut Vec<u8>, src_addr: SocketAddr, dst_addr: SocketAddr) {
    ExportedPduDissectorHint::serialize_exported_pdu(hdr);

    push_addr(hdr, src_addr, dst_addr);

    // end of option
    hdr.extend_from_slice(&[0x00, EXP_PDU_TAG_END_OF_OPT, 0x00, 0x00]);
}

fn push_fixed_tcp_header(
    hdr: &mut Vec<u8>,
    src_addr: SocketAddr,
    dst_addr: SocketAddr,
    dissector_hint: &ExportedPduDissectorHint,
) {
    dissector_hint.serialize(hdr);

    push_addr(hdr, src_addr, dst_addr);

    // port type = tcp
    hdr.extend_from_slice(&[
        0x00,
        EXP_PDU_TAG_PORT_TYPE,
        0x00,
        EXP_PDU_TAG_PORT_TYPE_LEN,
        0x00,
        0x00,
        0x00,
        EXP_PDU_PT_TCP,
    ]);
}

const EXP_PDU_TAG_TCP_INFO_DATA: u8 = 34;
const EXP_PDU_TAG_END_OF_OPT: u8 = 0;

fn push_var_tcp_header(buf: &mut Vec<u8>) {
    // tcp dissector data
    buf.extend_from_slice(&[0x00, EXP_PDU_TAG_TCP_INFO_DATA, 0x00, 0x13, 0x00, 0x01]); // version 1
    buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // seq
    buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // next seq
    buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // last ack
    buf.extend_from_slice(&[0x01, 0x00, 0x18, 0x00, 0x00]); // reassembled, flags 0x0018

    // end of option
    buf.extend_from_slice(&[0x00, EXP_PDU_TAG_END_OF_OPT, 0x00, 0x00]);
}
