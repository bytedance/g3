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

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlpnProtocol {
    Http10,
    Http11,
    Http2,
    Http3,
}

impl fmt::Display for AlpnProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl AlpnProtocol {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Http10 => "http/1.0",
            Self::Http11 => "http/1.1",
            Self::Http2 => "h2",
            Self::Http3 => "h3",
        }
    }

    pub fn wired_identification_sequence(&self) -> &'static [u8] {
        match self {
            Self::Http10 => b"\x08http/1.0",
            Self::Http11 => b"\x08http/1.1",
            Self::Http2 => b"\x02h2",
            Self::Http3 => b"\x02h3",
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

    pub fn from_buf(buf: &[u8]) -> Option<Self> {
        match buf {
            b"http/1.0" => Some(AlpnProtocol::Http10),
            b"http/1.1" => Some(AlpnProtocol::Http11),
            b"h2" => Some(AlpnProtocol::Http2),
            b"h3" => Some(AlpnProtocol::Http3),
            _ => None,
        }
    }
}
