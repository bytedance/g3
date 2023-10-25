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

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt};

use super::{ProxyAddr, ProxyProtocolReadError};

const PROXY_HDR_V2_LEN: usize = 16;
const PROXY_DATA_V2_MAX_LEN: usize = 536;

const V2_MAGIC_HEADER: &[u8] = b"\x0d\x0a\x0d\x0a\x00\x0d\x0a\x51\x55\x49\x54\x0a";

const COMMAND_LOCAL: u8 = 0x00;
const COMMAND_PROXY: u8 = 0x01;

const FAMILY_UNSPEC: u8 = 0x00;
const FAMILY_INET: u8 = 0x01;
const FAMILY_INET6: u8 = 0x02;
const FAMILY_UNIX: u8 = 0x03;

const PROTOCOL_UNSPEC: u8 = 0x00;
const PROTOCOL_STREAM: u8 = 0x01;
const PROTOCOL_DGRAM: u8 = 0x02;

pub struct ProxyProtocolV2Reader {
    timeout: Duration,
    hdr_buf: [u8; PROXY_HDR_V2_LEN],
    data_buf: Box<[u8; PROXY_DATA_V2_MAX_LEN]>,
}

impl ProxyProtocolV2Reader {
    pub fn new(timeout: Duration) -> Self {
        ProxyProtocolV2Reader {
            timeout,
            hdr_buf: Default::default(),
            data_buf: Box::new([0u8; PROXY_DATA_V2_MAX_LEN]),
        }
    }

    pub async fn read_proxy_protocol_v2_for_tcp<R>(
        &mut self,
        reader: &mut R,
    ) -> Result<Option<ProxyAddr>, ProxyProtocolReadError>
    where
        R: AsyncRead + Unpin,
    {
        let data_len = match tokio::time::timeout(self.timeout, self.read_in_data(reader)).await {
            Ok(Ok(l)) => l,
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(ProxyProtocolReadError::ReadTimeout),
        };

        match self.command() {
            COMMAND_PROXY => {}
            COMMAND_LOCAL => return Ok(None),
            c => return Err(ProxyProtocolReadError::InvalidCommand(c)),
        }

        match self.protocol() {
            PROTOCOL_UNSPEC => return Ok(None),
            PROTOCOL_STREAM => {}
            PROTOCOL_DGRAM => return Err(ProxyProtocolReadError::InvalidProtocol(PROTOCOL_DGRAM)),
            p => return Err(ProxyProtocolReadError::InvalidProtocol(p)),
        }

        match self.family() {
            FAMILY_UNSPEC => Ok(None),
            FAMILY_INET => {
                let addr = self.get_inet_addr(data_len)?;
                Ok(Some(addr))
            }
            FAMILY_INET6 => {
                let addr = self.get_inet6_addr(data_len)?;
                Ok(Some(addr))
            }
            FAMILY_UNIX => Err(ProxyProtocolReadError::InvalidFamily(FAMILY_UNIX)),
            f => Err(ProxyProtocolReadError::InvalidFamily(f)),
        }
    }

    fn get_inet_addr(&self, data_len: usize) -> Result<ProxyAddr, ProxyProtocolReadError> {
        if data_len < 12 {
            return Err(ProxyProtocolReadError::InvalidDataLength(data_len));
        }

        let b = &self.data_buf[0..12];
        let src_addr = Ipv4Addr::from([b[0], b[1], b[2], b[3]]);
        let dst_addr = Ipv4Addr::from([b[4], b[5], b[6], b[7]]);
        let src_port = u16::from_be_bytes([b[8], b[9]]);
        let dst_port = u16::from_be_bytes([b[10], b[11]]);

        Ok(ProxyAddr {
            src_addr: SocketAddr::new(IpAddr::V4(src_addr), src_port),
            dst_addr: SocketAddr::new(IpAddr::V4(dst_addr), dst_port),
        })
    }

    fn get_inet6_addr(&self, data_len: usize) -> Result<ProxyAddr, ProxyProtocolReadError> {
        if data_len < 36 {
            return Err(ProxyProtocolReadError::InvalidDataLength(data_len));
        }

        let b = &self.data_buf[0..36];
        let src_addr = Ipv6Addr::from([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13],
            b[14], b[15],
        ]);
        let dst_addr = Ipv6Addr::from([
            b[16], b[17], b[18], b[19], b[20], b[21], b[22], b[23], b[24], b[25], b[26], b[27],
            b[28], b[29], b[30], b[31],
        ]);
        let src_port = u16::from_be_bytes([b[32], b[33]]);
        let dst_port = u16::from_be_bytes([b[34], b[35]]);

        Ok(ProxyAddr {
            src_addr: SocketAddr::new(IpAddr::V6(src_addr), src_port),
            dst_addr: SocketAddr::new(IpAddr::V6(dst_addr), dst_port),
        })
    }

    async fn read_in_data<R>(&mut self, reader: &mut R) -> Result<usize, ProxyProtocolReadError>
    where
        R: AsyncRead + Unpin,
    {
        let nr = reader.read_exact(&mut self.hdr_buf).await?;
        if nr != PROXY_HDR_V2_LEN {
            return Err(ProxyProtocolReadError::ClosedUnexpected);
        }

        if &self.hdr_buf[0..V2_MAGIC_HEADER.len()] != V2_MAGIC_HEADER {
            return Err(ProxyProtocolReadError::InvalidMagicHeader);
        }

        match self.version() {
            0x02 => {}
            v => return Err(ProxyProtocolReadError::InvalidVersion(v)),
        }

        let data_len = u16::from_be_bytes([self.hdr_buf[14], self.hdr_buf[15]]) as usize;
        if data_len > self.data_buf.len() {
            return Err(ProxyProtocolReadError::InvalidDataLength(data_len));
        }

        let nr = reader.read_exact(&mut self.data_buf[0..data_len]).await?;
        if nr != data_len {
            Err(ProxyProtocolReadError::ClosedUnexpected)
        } else {
            Ok(data_len)
        }
    }

    #[inline]
    fn version(&self) -> u8 {
        self.hdr_buf[12] >> 4
    }

    #[inline]
    fn command(&self) -> u8 {
        self.hdr_buf[12] & 0x0F
    }

    #[inline]
    fn family(&self) -> u8 {
        self.hdr_buf[13] >> 4
    }

    #[inline]
    fn protocol(&self) -> u8 {
        self.hdr_buf[13] & 0x0F
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use g3_types::net::{ProxyProtocolEncoder, ProxyProtocolVersion};
    use std::io;
    use std::str::FromStr;
    use tokio_util::io::StreamReader;

    async fn run_t(client: SocketAddr, server: SocketAddr) {
        let mut encoder = ProxyProtocolEncoder::new(ProxyProtocolVersion::V2);
        let encoded = encoder.encode_tcp(client, server).unwrap();

        let stream = tokio_stream::iter(vec![<io::Result<Bytes>>::Ok(Bytes::copy_from_slice(
            encoded,
        ))]);
        let mut stream = StreamReader::new(stream);

        let mut reader = ProxyProtocolV2Reader::new(Duration::from_secs(1));
        let addr = reader
            .read_proxy_protocol_v2_for_tcp(&mut stream)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(addr.src_addr, client);
        assert_eq!(addr.dst_addr, server);
    }

    #[tokio::test]
    async fn t_tcp4() {
        let client = SocketAddr::from_str("192.168.0.1:56324").unwrap();
        let server = SocketAddr::from_str("192.168.0.11:443").unwrap();

        run_t(client, server).await;
    }

    #[tokio::test]
    async fn t_tcp6() {
        let client = SocketAddr::from_str("[2001:db8::1]:56324").unwrap();
        let server = SocketAddr::from_str("[2001:db8::11]:443").unwrap();

        run_t(client, server).await;
    }
}
