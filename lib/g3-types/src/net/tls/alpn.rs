/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::cmp::Ordering;
use std::fmt;

use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlpnProtocol {
    Http10,
    Http11,
    Http2,
    Http3,
    Ftp,
    Smtp,
    Imap,
    Pop3,
    Nntp,
    Nnsp,
    Mqtt,
    DnsOverTls,
    DnsOverQuic,
}

impl fmt::Display for AlpnProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AlpnProtocol {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Http10 => "http/1.0",
            Self::Http11 => "http/1.1",
            Self::Http2 => "h2",
            Self::Http3 => "h3",
            Self::Ftp => "ftp",
            Self::Smtp => "smtp", // not a IANA standard, but at least exim use this
            Self::Imap => "imap",
            Self::Pop3 => "pop3",
            Self::Nntp => "nntp",
            Self::Nnsp => "nnsp",
            Self::Mqtt => "mqtt",
            Self::DnsOverTls => "dot",
            Self::DnsOverQuic => "doq",
        }
    }

    pub const fn wired_identification_sequence(&self) -> &'static [u8] {
        match self {
            Self::Http10 => b"\x08http/1.0",
            Self::Http11 => b"\x08http/1.1",
            Self::Http2 => b"\x02h2",
            Self::Http3 => b"\x02h3",
            Self::Ftp => b"\x03ftp",
            Self::Smtp => b"\x04smtp",
            Self::Imap => b"\x04imap",
            Self::Pop3 => b"\x04pop3",
            Self::Nntp => b"\x04nntp",
            Self::Nnsp => b"\x04nnsp",
            Self::Mqtt => b"\x04mqtt",
            Self::DnsOverTls => b"\x03dot",
            Self::DnsOverQuic => b"\x03doq",
        }
    }

    #[inline]
    pub fn identification_sequence(&self) -> &'static [u8] {
        &self.wired_identification_sequence()[1..]
    }

    #[inline]
    pub fn to_identification_sequence(&self) -> Vec<u8> {
        self.identification_sequence().to_vec()
    }

    pub fn from_selected(buf: &[u8]) -> Option<Self> {
        match buf {
            b"http/1.0" => Some(AlpnProtocol::Http10),
            b"http/1.1" => Some(AlpnProtocol::Http11),
            b"h2" => Some(AlpnProtocol::Http2),
            b"h3" => Some(AlpnProtocol::Http3),
            b"ftp" => Some(AlpnProtocol::Ftp),
            b"smtp" => Some(AlpnProtocol::Smtp),
            b"imap" => Some(AlpnProtocol::Imap),
            b"pop3" => Some(AlpnProtocol::Pop3),
            b"nntp" => Some(AlpnProtocol::Nntp),
            b"nnsp" => Some(AlpnProtocol::Nnsp),
            b"mqtt" => Some(AlpnProtocol::Mqtt),
            b"dot" => Some(AlpnProtocol::DnsOverTls),
            b"doq" => Some(AlpnProtocol::DnsOverQuic),
            _ => None,
        }
    }
}

#[derive(Debug, Error)]
pub enum TlsAlpnError {
    #[error("not enough data: {0}")]
    NotEnoughData(usize),
    #[error("invalid list length {0}")]
    InvalidListLength(u16),
    #[error("empty protocol name")]
    EmptyProtocolName,
    #[error("truncated protocol name")]
    TruncatedProtocolName,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TlsAlpn {
    raw_list: Vec<u8>,
}

impl TlsAlpn {
    pub fn from_extension_value(buf: &[u8]) -> Result<TlsAlpn, TlsAlpnError> {
        let buf_len = buf.len();
        if buf_len < 2 {
            return Err(TlsAlpnError::NotEnoughData(buf_len));
        }

        let list_len = u16::from_be_bytes([buf[0], buf[1]]);
        if list_len as usize + 2 != buf_len {
            return Err(TlsAlpnError::InvalidListLength(list_len));
        }

        let mut offset = 2;
        loop {
            match buf.len().cmp(&offset) {
                Ordering::Equal => break,
                Ordering::Less => return Err(TlsAlpnError::TruncatedProtocolName),
                Ordering::Greater => {
                    let name_len = buf[offset] as usize;
                    if name_len == 0 {
                        return Err(TlsAlpnError::EmptyProtocolName);
                    }
                    offset += 1 + name_len;
                }
            }
        }

        Ok(TlsAlpn {
            raw_list: Vec::from(&buf[2..]),
        })
    }

    #[inline]
    pub fn wired_list_sequence(&self) -> &[u8] {
        self.raw_list.as_slice()
    }

    pub fn retain_clone<F>(&self, retain: F) -> Self
    where
        F: Fn(&[u8]) -> bool,
    {
        let mut new = Vec::with_capacity(self.raw_list.len());
        let mut offset = 0usize;

        while offset < self.raw_list.len() {
            let len = self.raw_list[offset] as usize;
            if offset + len > self.raw_list.len() {
                break;
            }
            let start = offset + 1;
            let end = start + len;
            if retain(&self.raw_list[start..end]) {
                new.extend_from_slice(&self.raw_list[offset..end]);
            }
            offset = end;
        }

        TlsAlpn { raw_list: new }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.raw_list.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filer() {
        let v = b"\x00\x0C\x02h2\x08http/1.0";

        let alpn = TlsAlpn::from_extension_value(v).unwrap();
        let filtered = alpn.retain_clone(|b| b != b"h2");
        assert!(!filtered.is_empty());

        let v = b"\x00\x09\x08http/1.0";
        let alpn2 = TlsAlpn::from_extension_value(v).unwrap();

        assert_eq!(filtered, alpn2);
    }
}
