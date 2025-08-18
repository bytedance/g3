/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use bytes::BufMut;
use http::HeaderName;

pub const fn connection_as_bytes(close: bool) -> &'static [u8] {
    if close {
        b"Connection: Close\r\n"
    } else {
        b"Connection: Keep-Alive\r\n"
    }
}

#[derive(Clone, Default)]
pub enum Connection {
    #[default]
    CamelCase,
    LowerCase,
    UpperCase,
}

impl Connection {
    pub fn from_original(original: &str) -> Self {
        match original {
            "Connection" | "Proxy-Connection" => Connection::CamelCase,
            "connection" | "proxy-connection" => Connection::LowerCase,
            "CONNECTION" | "PROXY-CONNECTION" => Connection::UpperCase,
            _ => Connection::CamelCase,
        }
    }

    fn as_str(&self) -> &str {
        match self {
            Connection::CamelCase => "Connection",
            Connection::LowerCase => "connection",
            Connection::UpperCase => "CONNECTION",
        }
    }

    pub fn write_to_buf(&self, close: bool, headers: &[HeaderName], buf: &mut Vec<u8>) {
        buf.put_slice(self.as_str().as_bytes());
        buf.put_slice(b": ");
        if close {
            buf.put_slice(b"Close");
        } else {
            buf.put_slice(b"Keep-Alive");
        }
        for h in headers {
            buf.put_slice(b", ");
            buf.put_slice(h.as_str().as_bytes());
        }
        buf.put_slice(b"\r\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_connection_as_bytes() {
        assert_eq!(connection_as_bytes(true), b"Connection: Close\r\n");
        assert_eq!(connection_as_bytes(false), b"Connection: Keep-Alive\r\n");
    }

    #[test]
    fn connection_from_original() {
        assert!(matches!(
            Connection::from_original("Connection"),
            Connection::CamelCase
        ));
        assert!(matches!(
            Connection::from_original("connection"),
            Connection::LowerCase
        ));
        assert!(matches!(
            Connection::from_original("CONNECTION"),
            Connection::UpperCase
        ));
        assert!(matches!(
            Connection::from_original("invalid"),
            Connection::CamelCase
        ));
    }

    #[test]
    fn connection_as_str() {
        assert_eq!(Connection::CamelCase.as_str(), "Connection");
        assert_eq!(Connection::LowerCase.as_str(), "connection");
        assert_eq!(Connection::UpperCase.as_str(), "CONNECTION");
    }

    #[test]
    fn connection_write_to_buf() {
        let mut buf = Vec::new();
        let headers = [
            HeaderName::from_static("upgrade"),
            HeaderName::from_static("content-length"),
        ];

        // close=true and headers
        Connection::CamelCase.write_to_buf(true, &headers, &mut buf);
        assert_eq!(buf, b"Connection: Close, upgrade, content-length\r\n");
        buf.clear();

        // close=false and no headers
        Connection::LowerCase.write_to_buf(false, &[], &mut buf);
        assert_eq!(buf, b"connection: Keep-Alive\r\n");
        buf.clear();

        // close=false and headers (uppercase variant)
        Connection::UpperCase.write_to_buf(false, &headers, &mut buf);
        assert_eq!(buf, b"CONNECTION: Keep-Alive, upgrade, content-length\r\n");
    }
}
