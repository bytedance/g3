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

const V2_MAGIC_HEADER: &[u8] = b"\x0d\x0a\x0d\x0a\x00\x0d\x0a\x51\x55\x49\x54\x0a";

const V2_BUF_CAP: usize = 52;

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

pub struct ProxyProtocolV2Encoder([u8; V2_BUF_CAP]);

impl ProxyProtocolV2Encoder {
    pub fn new() -> Self {
        ProxyProtocolV2Encoder([0u8; V2_BUF_CAP])
    }

    pub fn encode_tcp(
        &mut self,
        client_addr: SocketAddr,
        server_addr: SocketAddr,
    ) -> Result<&[u8], ProxyProtocolEncodeError> {
        self.0[..12].copy_from_slice(V2_MAGIC_HEADER);

        match (client_addr, server_addr) {
            (SocketAddr::V4(c4), SocketAddr::V4(s4)) => {
                self.0[12..16].copy_from_slice(&[BYTE_13_PROXY, BYTE14_TCP4, 0x00, 12]);
                self.0[16..20].copy_from_slice(&c4.ip().octets());
                self.0[20..24].copy_from_slice(&s4.ip().octets());
                self.0[24..26].copy_from_slice(&c4.port().to_be_bytes());
                self.0[26..28].copy_from_slice(&s4.port().to_be_bytes());
                Ok(&self.0[..28])
            }
            (SocketAddr::V6(c6), SocketAddr::V6(s6)) => {
                self.0[12..16].copy_from_slice(&[BYTE_13_PROXY, BYTE14_TCP6, 0x00, 36]);
                self.0[16..32].copy_from_slice(&c6.ip().octets());
                self.0[32..48].copy_from_slice(&s6.ip().octets());
                self.0[48..50].copy_from_slice(&c6.port().to_be_bytes());
                self.0[50..52].copy_from_slice(&s6.port().to_be_bytes());
                Ok(&self.0[..52])
            }
            _ => Err(ProxyProtocolEncodeError::AddressFamilyNotMatch),
        }
    }
}

impl Default for ProxyProtocolV2Encoder {
    fn default() -> Self {
        ProxyProtocolV2Encoder::new()
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

        let mut encoder = ProxyProtocolV2Encoder::new();
        let encoded = encoder.encode_tcp(client, server).unwrap();
        assert_eq!(
            encoded,
            b"\x0d\x0a\x0d\x0a\x00\x0d\x0a\x51\x55\x49\x54\x0a\
              \x21\x11\x00\x0C\
              \xC0\xA8\x00\x01\
              \xC0\xA8\x00\x0B\
              \xDC\x04\x01\xBB"
        );
    }

    #[test]
    fn t_tcp6() {
        let client = SocketAddr::from_str("[2001:db8::1]:56324").unwrap();
        let server = SocketAddr::from_str("[2001:db8::11]:443").unwrap();

        let mut encoder = ProxyProtocolV2Encoder::new();
        let encoded = encoder.encode_tcp(client, server).unwrap();
        assert_eq!(
            encoded,
            b"\x0d\x0a\x0d\x0a\x00\x0d\x0a\x51\x55\x49\x54\x0a\
              \x21\x21\x00\x24\
              \x20\x01\x0d\xb8\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x01\
              \x20\x01\x0d\xb8\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x11\
              \xDC\x04\x01\xBB"
        );
    }
}
