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

use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

use http::HeaderName;

use crate::net::{HttpHeaderMap, HttpHeaderValue};

#[derive(Clone, Copy, Debug)]
pub struct HttpStandardForwardedHeaderValue {
    for_addr: SocketAddr,
    by_addr: SocketAddr,
}

#[derive(Clone, Copy, Debug)]
pub enum HttpForwardedHeaderValue {
    Classic(IpAddr),
    Standard(HttpStandardForwardedHeaderValue),
}

impl HttpForwardedHeaderValue {
    pub fn new_classic(ip: IpAddr) -> Self {
        HttpForwardedHeaderValue::Classic(ip)
    }

    pub fn new_standard(for_addr: SocketAddr, by_addr: SocketAddr) -> Self {
        HttpForwardedHeaderValue::Standard(HttpStandardForwardedHeaderValue { for_addr, by_addr })
    }

    pub fn append_to(&self, map: &mut HttpHeaderMap) {
        match self {
            HttpForwardedHeaderValue::Classic(ip) => {
                let name = HeaderName::from_static("x-forwarded-for");
                map.append(name, unsafe {
                    HttpHeaderValue::from_string_unchecked(ip.to_string())
                });
            }
            HttpForwardedHeaderValue::Standard(HttpStandardForwardedHeaderValue {
                for_addr,
                by_addr,
            }) => {
                let s = match (for_addr, by_addr) {
                    (SocketAddr::V4(f), SocketAddr::V4(b)) => {
                        format!("for={f}; by={b}")
                    }
                    (SocketAddr::V4(f), SocketAddr::V6(b)) => {
                        format!("for={f}; by=\"{b}\"")
                    }
                    (SocketAddr::V6(f), SocketAddr::V4(b)) => {
                        format!("for=\"{f}\"; by={b}")
                    }
                    (SocketAddr::V6(f), SocketAddr::V6(b)) => {
                        format!("for=\"{f}\"; by=\"{b}\"")
                    }
                };
                map.append(http::header::FORWARDED, unsafe {
                    HttpHeaderValue::from_string_unchecked(s)
                });
            }
        }
    }

    pub fn build_header_line(&self) -> String {
        match self {
            HttpForwardedHeaderValue::Classic(ip) => {
                format!("X-Forwarded-For: {ip}\r\n")
            }
            HttpForwardedHeaderValue::Standard(HttpStandardForwardedHeaderValue {
                for_addr,
                by_addr,
            }) => match (for_addr, by_addr) {
                (SocketAddr::V4(f), SocketAddr::V4(b)) => {
                    format!("Forwarded: for={f}; by={b}\r\n")
                }
                (SocketAddr::V4(f), SocketAddr::V6(b)) => {
                    format!("Forwarded: for={f}; by=\"{b}\"\r\n")
                }
                (SocketAddr::V6(f), SocketAddr::V4(b)) => {
                    format!("Forwarded: for=\"{f}\"; by={b}\r\n")
                }
                (SocketAddr::V6(f), SocketAddr::V6(b)) => {
                    format!("Forwarded: for=\"{f}\"; by=\"{b}\"\r\n")
                }
            },
        }
    }
}

#[derive(Clone, Copy, Default, Debug, Eq, PartialEq)]
pub enum HttpForwardedHeaderType {
    #[default]
    Classic,
    Standard,
    Disable,
}

impl FromStr for HttpForwardedHeaderType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "none" | "disable" => Ok(HttpForwardedHeaderType::Disable),
            "classic" | "enable" => Ok(HttpForwardedHeaderType::Classic),
            "standard" | "rfc7239" => Ok(HttpForwardedHeaderType::Standard),
            _ => Err(()),
        }
    }
}
