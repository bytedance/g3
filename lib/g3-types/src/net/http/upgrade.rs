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
use std::str::FromStr;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum HttpUpgradeTokenParseError {
    #[error("unsupported protocol {0}")]
    UnsupportedProtocol(String),
    #[error("unsupported version for {0}")]
    UnsupportedVersion(&'static str),
    #[error("version is required for {0}")]
    VersionIsRequired(&'static str),
}

#[derive(Clone, Debug)]
pub enum HttpUpgradeToken {
    Http(http::Version),
    Tls(u8, u8),
    Websocket,
    ConnectUdp,
    Unsupported(String),
}

impl FromStr for HttpUpgradeToken {
    type Err = HttpUpgradeTokenParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split_once('/') {
            Some((p, v)) => match p.to_lowercase().as_str() {
                "http" => match v {
                    "1.0" => Ok(HttpUpgradeToken::Http(http::Version::HTTP_10)),
                    "1.1" => Ok(HttpUpgradeToken::Http(http::Version::HTTP_11)),
                    "2.0" => Ok(HttpUpgradeToken::Http(http::Version::HTTP_2)),
                    _ => Err(HttpUpgradeTokenParseError::UnsupportedVersion("http")),
                },
                "tls" => match v {
                    "1.0" => Ok(HttpUpgradeToken::Tls(1, 0)),
                    "1.1" => Ok(HttpUpgradeToken::Tls(1, 1)),
                    "1.2" => Ok(HttpUpgradeToken::Tls(1, 2)),
                    "1.3" => Ok(HttpUpgradeToken::Tls(1, 3)),
                    _ => Err(HttpUpgradeTokenParseError::UnsupportedVersion("tls")),
                },
                _ => Err(HttpUpgradeTokenParseError::UnsupportedProtocol(
                    p.to_string(),
                )),
            },
            None => match s.to_lowercase().as_str() {
                "http" => Ok(HttpUpgradeToken::Http(http::Version::HTTP_11)),
                "tls" => Err(HttpUpgradeTokenParseError::VersionIsRequired("tls")),
                "websocket" => Ok(HttpUpgradeToken::Websocket),
                "connect-udp" => Ok(HttpUpgradeToken::ConnectUdp),
                _ => Err(HttpUpgradeTokenParseError::UnsupportedProtocol(
                    s.to_string(),
                )),
            },
        }
    }
}

impl fmt::Display for HttpUpgradeToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HttpUpgradeToken::Http(v) => write!(f, "{v:?}"),
            HttpUpgradeToken::Tls(major, minor) => write!(f, "TLS/{major}.{minor}"),
            HttpUpgradeToken::Websocket => write!(f, "websocket"),
            HttpUpgradeToken::ConnectUdp => write!(f, "connect-udp"),
            HttpUpgradeToken::Unsupported(s) => write!(f, "{s}"),
        }
    }
}
