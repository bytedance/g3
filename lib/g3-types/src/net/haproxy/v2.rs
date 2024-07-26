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

use std::net::SocketAddr;

use super::ProxyProtocolEncodeError;
use crate::net::{Host, UpstreamAddr};

const V2_MAGIC_HEADER: &[u8] = b"\x0d\x0a\x0d\x0a\x00\x0d\x0a\x51\x55\x49\x54\x0a";

const V2_BUF_CAP: usize = 536;
const V2_HDR_LEN: usize = 16;

const BITS_VERSION: u8 = 0x20;

const _SOURCE_LOCAL: u8 = 0x00;
const SOURCE_PROXY: u8 = 0x01;

const BYTE_13_PROXY: u8 = BITS_VERSION | SOURCE_PROXY;

const _AF_UNSPEC: u8 = 0x00;
const AF_INET: u8 = 0x10;
const AF_INET6: u8 = 0x20;
const _AF_UNIX: u8 = 0x30;

const _PROTO_UNSPEC: u8 = 0x00;
const PROTO_STREAM: u8 = 0x01;
const _PROTO_DGRAM: u8 = 0x02;

const BYTE14_TCP4: u8 = AF_INET | PROTO_STREAM;
const BYTE14_TCP6: u8 = AF_INET6 | PROTO_STREAM;

// TODO use concat_bytes to generate header after it's stabilization
// const V2_HEADER_TCP4: &[u8] = concat_bytes!(V2_MAGIC_HEADER, &[BYTE_13_PROXY, BYTE14_TCP4, 0x00, 12]);
// const V2_HEADER_TCP6: &[u8] = concat_bytes!(V2_MAGIC_HEADER, &[BYTE_13_PROXY, BYTE14_TCP6, 0x00, 36]);

const PP2_TYPE_CUSTOM_UPSTREAM: u8 = 0xE0;
const PP2_TYPE_CUSTOM_TLS_NAME: u8 = 0xE1;
const PP2_TYPE_CUSTOM_USERNAME: u8 = 0xE2;
const PP2_TYPE_CUSTOM_TASK_ID: u8 = 0xE3;

pub struct ProxyProtocolV2Encoder {
    buf: [u8; V2_BUF_CAP],
    len: usize,
}

impl ProxyProtocolV2Encoder {
    pub(super) fn new() -> Self {
        ProxyProtocolV2Encoder {
            buf: [0u8; V2_BUF_CAP],
            len: 0,
        }
    }

    pub fn new_tcp(
        client_addr: SocketAddr,
        server_addr: SocketAddr,
    ) -> Result<Self, ProxyProtocolEncodeError> {
        let mut encoder = ProxyProtocolV2Encoder::new();
        encoder.encode_tcp(client_addr, server_addr)?;
        Ok(encoder)
    }

    pub(super) fn encode_tcp(
        &mut self,
        client_addr: SocketAddr,
        server_addr: SocketAddr,
    ) -> Result<&[u8], ProxyProtocolEncodeError> {
        self.buf[..12].copy_from_slice(V2_MAGIC_HEADER);

        match (client_addr, server_addr) {
            (SocketAddr::V4(c4), SocketAddr::V4(s4)) => {
                self.buf[12..16].copy_from_slice(&[BYTE_13_PROXY, BYTE14_TCP4, 0, 12]);
                self.buf[16..20].copy_from_slice(&c4.ip().octets());
                self.buf[20..24].copy_from_slice(&s4.ip().octets());
                self.buf[24..26].copy_from_slice(&c4.port().to_be_bytes());
                self.buf[26..28].copy_from_slice(&s4.port().to_be_bytes());
                self.len = 28;
                Ok(&self.buf[..self.len])
            }
            (SocketAddr::V6(c6), SocketAddr::V6(s6)) => {
                self.buf[12..16].copy_from_slice(&[BYTE_13_PROXY, BYTE14_TCP6, 0, 36]);
                self.buf[16..32].copy_from_slice(&c6.ip().octets());
                self.buf[32..48].copy_from_slice(&s6.ip().octets());
                self.buf[48..50].copy_from_slice(&c6.port().to_be_bytes());
                self.buf[50..52].copy_from_slice(&s6.port().to_be_bytes());
                self.len = 52;
                Ok(&self.buf[..self.len])
            }
            _ => Err(ProxyProtocolEncodeError::AddressFamilyNotMatch),
        }
    }

    pub fn push_tlv(&mut self, key: u8, value: &[u8]) -> Result<(), ProxyProtocolEncodeError> {
        let v_len = value.len();
        let len = u16::try_from(value.len()).map_err(ProxyProtocolEncodeError::InvalidU16Length)?;
        let len_b = len.to_be_bytes();
        let mut offset = self.len;
        self.len += 3 + v_len;
        if self.len > V2_BUF_CAP {
            self.len = offset;
            return Err(ProxyProtocolEncodeError::TotalLengthOverflow);
        }
        self.buf[offset] = key;
        offset += 1;
        self.buf[offset..offset + 2].copy_from_slice(&len_b);
        offset += 2;
        self.buf[offset..offset + v_len].copy_from_slice(value);
        Ok(())
    }

    pub fn push_upstream(
        &mut self,
        upstream: &UpstreamAddr,
    ) -> Result<(), ProxyProtocolEncodeError> {
        let value = upstream.to_string();
        self.push_tlv(PP2_TYPE_CUSTOM_UPSTREAM, value.as_bytes())
    }

    pub fn push_tls_name(&mut self, tls_name: &Host) -> Result<(), ProxyProtocolEncodeError> {
        match tls_name {
            Host::Domain(s) => self.push_tlv(PP2_TYPE_CUSTOM_TLS_NAME, s.as_bytes()),
            Host::Ip(ip) => {
                let ip = ip.to_string();
                self.push_tlv(PP2_TYPE_CUSTOM_TLS_NAME, ip.as_bytes())
            }
        }
    }

    pub fn push_username(&mut self, name: &str) -> Result<(), ProxyProtocolEncodeError> {
        self.push_tlv(PP2_TYPE_CUSTOM_USERNAME, name.as_bytes())
    }

    pub fn push_task_id(&mut self, id: &[u8]) -> Result<(), ProxyProtocolEncodeError> {
        self.push_tlv(PP2_TYPE_CUSTOM_TASK_ID, id)
    }

    pub fn finalize(&mut self) -> &[u8] {
        let data_len = (self.len - V2_HDR_LEN) as u16; // won't overlap
        let b = data_len.to_be_bytes();
        self.buf[14..=15].copy_from_slice(&b);
        &self.buf[..self.len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn t_tcp4() {
        let client = SocketAddr::from_str("192.168.0.1:56324").unwrap();
        let server = SocketAddr::from_str("192.168.0.11:443").unwrap();

        let mut encoder = ProxyProtocolV2Encoder::new_tcp(client, server).unwrap();
        assert_eq!(
            encoder.finalize(),
            b"\x0d\x0a\x0d\x0a\x00\x0d\x0a\x51\x55\x49\x54\x0a\
              \x21\x11\x00\x0C\
              \xC0\xA8\x00\x01\
              \xC0\xA8\x00\x0B\
              \xDC\x04\x01\xBB"
        );
    }

    #[test]
    fn t_tcp4_tlv() {
        let client = SocketAddr::from_str("192.168.0.1:56324").unwrap();
        let server = SocketAddr::from_str("192.168.0.11:443").unwrap();

        let mut encoder = ProxyProtocolV2Encoder::new_tcp(client, server).unwrap();
        encoder.push_task_id(b"1234").unwrap();
        assert_eq!(
            encoder.finalize(),
            b"\x0d\x0a\x0d\x0a\x00\x0d\x0a\x51\x55\x49\x54\x0a\
              \x21\x11\x00\x13\
              \xC0\xA8\x00\x01\
              \xC0\xA8\x00\x0B\
              \xDC\x04\x01\xBB\
              \xE3\x00\x04\
              1234"
        );
    }

    #[test]
    fn t_tcp6() {
        let client = SocketAddr::from_str("[2001:db8::1]:56324").unwrap();
        let server = SocketAddr::from_str("[2001:db8::11]:443").unwrap();

        let mut encoder = ProxyProtocolV2Encoder::new_tcp(client, server).unwrap();
        assert_eq!(
            encoder.finalize(),
            b"\x0d\x0a\x0d\x0a\x00\x0d\x0a\x51\x55\x49\x54\x0a\
              \x21\x21\x00\x24\
              \x20\x01\x0d\xb8\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x01\
              \x20\x01\x0d\xb8\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x11\
              \xDC\x04\x01\xBB"
        );
    }

    #[test]
    fn t_tcp6_tlv() {
        let client = SocketAddr::from_str("[2001:db8::1]:56324").unwrap();
        let server = SocketAddr::from_str("[2001:db8::11]:443").unwrap();

        let mut encoder = ProxyProtocolV2Encoder::new_tcp(client, server).unwrap();
        encoder.push_username("1234").unwrap();
        assert_eq!(
            encoder.finalize(),
            b"\x0d\x0a\x0d\x0a\x00\x0d\x0a\x51\x55\x49\x54\x0a\
              \x21\x21\x00\x2B\
              \x20\x01\x0d\xb8\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x01\
              \x20\x01\x0d\xb8\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x11\
              \xDC\x04\x01\xBB\
              \xE2\x00\x04\
              1234"
        );
    }
}
