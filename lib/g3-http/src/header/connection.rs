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
