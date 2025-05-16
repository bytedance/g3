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
