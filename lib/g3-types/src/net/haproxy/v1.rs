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

use std::io::Write;
use std::net::SocketAddr;

use super::ProxyProtocolEncodeError;

const V1_BUF_CAP: usize = 108;

pub struct ProxyProtocolV1Encoder(Vec<u8>);

impl ProxyProtocolV1Encoder {
    pub fn new() -> Self {
        ProxyProtocolV1Encoder(Vec::with_capacity(V1_BUF_CAP))
    }

    pub fn encode_tcp(
        &mut self,
        client_addr: SocketAddr,
        server_addr: SocketAddr,
    ) -> Result<&[u8], ProxyProtocolEncodeError> {
        self.0.clear();
        match (client_addr, server_addr) {
            (SocketAddr::V4(_), SocketAddr::V4(_)) => {
                self.0.extend_from_slice(b"PROXY TCP4 ");
            }
            (SocketAddr::V6(_), SocketAddr::V6(_)) => {
                self.0.extend_from_slice(b"PROXY TCP6 ");
            }
            _ => return Err(ProxyProtocolEncodeError::AddressFamilyNotMatch),
        }
        let _ = write!(
            self.0,
            "{} {} {} {}\r\n",
            client_addr.ip(),
            server_addr.ip(),
            client_addr.port(),
            server_addr.port()
        );
        Ok(self.0.as_slice())
    }
}

impl Default for ProxyProtocolV1Encoder {
    fn default() -> Self {
        ProxyProtocolV1Encoder::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn t_v4() {
        let client = SocketAddr::from_str("192.168.0.1:56324").unwrap();
        let server = SocketAddr::from_str("192.168.0.11:443").unwrap();

        let mut encoder = ProxyProtocolV1Encoder::new();
        let encoded = encoder.encode_tcp(client, server).unwrap();
        assert_eq!(
            encoded,
            "PROXY TCP4 192.168.0.1 192.168.0.11 56324 443\r\n".as_bytes()
        );
    }

    #[test]
    fn t_v6() {
        let client = SocketAddr::from_str("[2001:db8::1]:56324").unwrap();
        let server = SocketAddr::from_str("[2001:db8::11]:443").unwrap();

        let mut encoder = ProxyProtocolV1Encoder::new();
        let encoded = encoder.encode_tcp(client, server).unwrap();
        assert_eq!(
            encoded,
            "PROXY TCP6 2001:db8::1 2001:db8::11 56324 443\r\n".as_bytes()
        );
    }
}
