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

use g3_types::net::AlpnProtocol;

mod inspect;
use inspect::ProtocolInspectState;
pub use inspect::{ProtocolInspectError, ProtocolInspector};

mod portmap;
pub use portmap::{ProtocolPortMap, ProtocolPortMapValue};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u16)]
pub enum MaybeProtocol {
    Http,
    Smtp,
    Ssh,
    Ftp,
    Pop3,
    Nntp,
    Imap,
    Rtsp,
    Mqtt,
    Rtmp,
    Nats,
    BitTorrent,

    Https,
    Pop3s,
    Nntps,
    Imaps,
    Rtsps,
    SecureMqtt,
    Rtmps,

    Ssl,
}

impl MaybeProtocol {
    pub const fn is_ssl(&self) -> bool {
        matches!(
            self,
            MaybeProtocol::Ssl
                | MaybeProtocol::Https
                | MaybeProtocol::Pop3s
                | MaybeProtocol::Nntps
                | MaybeProtocol::Imaps
                | MaybeProtocol::Rtsps
                | MaybeProtocol::SecureMqtt
                | MaybeProtocol::Rtmps
        )
    }
}

impl FromStr for MaybeProtocol {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "http" => Ok(MaybeProtocol::Http),
            "smtp" => Ok(MaybeProtocol::Smtp),
            "ssh" => Ok(MaybeProtocol::Ssh),
            "ftp" => Ok(MaybeProtocol::Ftp),
            "pop3" => Ok(MaybeProtocol::Pop3),
            "nntp" => Ok(MaybeProtocol::Nntp),
            "imap" => Ok(MaybeProtocol::Imap),
            "rtsp" => Ok(MaybeProtocol::Rtsp),
            "rtmp" => Ok(MaybeProtocol::Rtmp),
            "nats" => Ok(MaybeProtocol::Nats),
            "bittorrent" | "bt" => Ok(MaybeProtocol::BitTorrent),
            "https" | "http+tls" => Ok(MaybeProtocol::Https),
            "pop3s" | "pop3+tls" => Ok(MaybeProtocol::Pop3s),
            "nntps" | "nntp+tls" => Ok(MaybeProtocol::Nntps),
            "imaps" | "imap+tls" => Ok(MaybeProtocol::Imaps),
            "rtsps" | "rtsp+tls" => Ok(MaybeProtocol::Rtsps),
            "secure-mqtt" => Ok(MaybeProtocol::SecureMqtt),
            "rtmps" | "rtmp+tls" => Ok(MaybeProtocol::Rtmps),
            "ssl" | "tls" => Ok(MaybeProtocol::Ssl),
            _ => Err(()),
        }
    }
}

impl From<AlpnProtocol> for MaybeProtocol {
    fn from(p: AlpnProtocol) -> Self {
        match p {
            AlpnProtocol::Http10 | AlpnProtocol::Http11 | AlpnProtocol::Http2 => {
                MaybeProtocol::Http
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Protocol {
    Unknown,
    SslLegacy,
    TlsLegacy,
    TlsModern,
    TlsTlcp,
    Http1,
    Http2,
    Smtp,
    SshLegacy,
    Ssh,
    FtpControl,
    Pop3,
    Nntp,
    Imap,
    Rtsp,
    Mqtt,
    Rtmp,
    Nats,
    BitTorrent,
    Websocket,
}

impl Protocol {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Protocol::Unknown => "unknown",
            Protocol::SslLegacy => "ssl_legacy",
            Protocol::TlsLegacy => "tls_legacy",
            Protocol::TlsModern => "tls_modern",
            Protocol::TlsTlcp => "tls_tlcp",
            Protocol::Http1 => "http_1",
            Protocol::Http2 => "http_2",
            Protocol::Smtp => "smtp",
            Protocol::SshLegacy => "ssh_legacy",
            Protocol::Ssh => "ssh",
            Protocol::FtpControl => "ftp_control",
            Protocol::Pop3 => "pop3",
            Protocol::Nntp => "nntp",
            Protocol::Imap => "imap",
            Protocol::Rtsp => "rtsp",
            Protocol::Mqtt => "mqtt",
            Protocol::Rtmp => "rtmp",
            Protocol::Nats => "nats",
            Protocol::BitTorrent => "bittorrent",
            Protocol::Websocket => "websocket",
        }
    }
}

impl From<AlpnProtocol> for Protocol {
    fn from(p: AlpnProtocol) -> Self {
        match p {
            AlpnProtocol::Http10 | AlpnProtocol::Http11 => Protocol::Http1,
            AlpnProtocol::Http2 => Protocol::Http2,
        }
    }
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

mod bittorrent;
mod ftp;
mod http;
mod imap;
mod mqtt;
mod nats;
mod nntp;
mod pop3;
mod rtmp;
mod rtsp;
mod smtp;
mod ssh;
mod ssl;
