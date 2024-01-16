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

use g3_types::net::{AlpnProtocol, WebSocketSubProtocol};

mod inspect;
use inspect::ProtocolInspectState;
pub use inspect::{ProtocolInspectError, ProtocolInspector};

mod portmap;
pub use portmap::{ProtocolPortMap, ProtocolPortMapValue};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(usize)]
pub enum MaybeProtocol {
    Http,
    Smtp,
    Ssh,
    Ftp,
    Dns,
    Pop3,
    Nntp,
    Nnsp,
    Imap,
    Rtsp,
    Mqtt,
    Stomp,
    Smpp,
    Rtmp,
    Nats,
    BitTorrent,

    Https,
    Pop3s,
    Nntps,
    Imaps,
    Rtsps,
    SecureMqtt,
    Ssmpp,
    Rtmps,
    DnsOverTls,

    Ssl,

    _MaxSize,
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
                | MaybeProtocol::Ssmpp
                | MaybeProtocol::Rtmps
                | MaybeProtocol::DnsOverTls
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
            "nnsp" => Ok(MaybeProtocol::Nnsp),
            "imap" => Ok(MaybeProtocol::Imap),
            "rtsp" => Ok(MaybeProtocol::Rtsp),
            "mqtt" => Ok(MaybeProtocol::Mqtt),
            "stomp" => Ok(MaybeProtocol::Stomp),
            "smpp" => Ok(MaybeProtocol::Smpp),
            "rtmp" => Ok(MaybeProtocol::Rtmp),
            "nats" => Ok(MaybeProtocol::Nats),
            "bittorrent" | "bt" => Ok(MaybeProtocol::BitTorrent),
            "https" | "http+tls" => Ok(MaybeProtocol::Https),
            "pop3s" | "pop3+tls" => Ok(MaybeProtocol::Pop3s),
            "nntps" | "nntp+tls" | "snntp" => Ok(MaybeProtocol::Nntps),
            "imaps" | "imap+tls" => Ok(MaybeProtocol::Imaps),
            "rtsps" | "rtsp+tls" => Ok(MaybeProtocol::Rtsps),
            "secure-mqtt" => Ok(MaybeProtocol::SecureMqtt),
            "ssmpp" | "smpps" | "secure smpp" => Ok(MaybeProtocol::Ssmpp),
            "rtmps" | "rtmp+tls" => Ok(MaybeProtocol::Rtmps),
            "dot" | "dnsovertls" | "dns-over-tls" => Ok(MaybeProtocol::DnsOverTls),
            "ssl" | "tls" => Ok(MaybeProtocol::Ssl),
            _ => Err(()),
        }
    }
}

impl From<AlpnProtocol> for MaybeProtocol {
    fn from(p: AlpnProtocol) -> Self {
        match p {
            AlpnProtocol::Http10
            | AlpnProtocol::Http11
            | AlpnProtocol::Http2
            | AlpnProtocol::Http3 => MaybeProtocol::Http,
            AlpnProtocol::Ftp => MaybeProtocol::Ftp,
            AlpnProtocol::Imap => MaybeProtocol::Imap,
            AlpnProtocol::Pop3 => MaybeProtocol::Pop3,
            AlpnProtocol::Nntp => MaybeProtocol::Nntp,
            AlpnProtocol::Nnsp => MaybeProtocol::Nnsp,
            AlpnProtocol::Mqtt => MaybeProtocol::Mqtt,
            AlpnProtocol::DnsOverTls => MaybeProtocol::Dns,
            AlpnProtocol::DnsOverQuic => MaybeProtocol::Dns,
        }
    }
}

impl From<WebSocketSubProtocol> for MaybeProtocol {
    fn from(p: WebSocketSubProtocol) -> Self {
        match p {
            WebSocketSubProtocol::Mqtt => MaybeProtocol::Mqtt,
            WebSocketSubProtocol::StompV10
            | WebSocketSubProtocol::StompV11
            | WebSocketSubProtocol::StompV12 => MaybeProtocol::Stomp,
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
    Http3,
    Smtp,
    SshLegacy,
    Ssh,
    FtpControl,
    Pop3,
    Nntp,
    Nnsp,
    Imap,
    Rtsp,
    Mqtt,
    Stomp,
    Smpp,
    RtmpOverTcp,
    RtmpOverHttp,
    Nats,
    BitTorrentOverTcp,
    BitTorrentOverUtp,
    Websocket,
    Dns,
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
            Protocol::Http3 => "http_3",
            Protocol::Smtp => "smtp",
            Protocol::SshLegacy => "ssh_legacy",
            Protocol::Ssh => "ssh",
            Protocol::FtpControl => "ftp_control",
            Protocol::Pop3 => "pop3",
            Protocol::Nntp => "nntp",
            Protocol::Nnsp => "nnsp",
            Protocol::Imap => "imap",
            Protocol::Rtsp => "rtsp",
            Protocol::Mqtt => "mqtt",
            Protocol::Stomp => "stomp",
            Protocol::Smpp => "smpp",
            Protocol::RtmpOverTcp | Protocol::RtmpOverHttp => "rtmp",
            Protocol::Nats => "nats",
            Protocol::BitTorrentOverTcp | Protocol::BitTorrentOverUtp => "bittorrent",
            Protocol::Websocket => "websocket",
            Protocol::Dns => "dns",
        }
    }

    pub const fn wireshark_dissector(&self) -> &'static str {
        match self {
            Protocol::Unknown => "",
            Protocol::SslLegacy | Protocol::TlsLegacy | Protocol::TlsModern => "tls",
            Protocol::TlsTlcp => "tls",
            Protocol::Http1 => "http",
            Protocol::Http2 => "http2",
            Protocol::Http3 => "http3",
            Protocol::Smtp => "smtp",
            Protocol::SshLegacy | Protocol::Ssh => "ssh",
            Protocol::FtpControl => "ftp",
            Protocol::Pop3 => "pop",
            Protocol::Nntp | Protocol::Nnsp => "nntp",
            Protocol::Imap => "imap",
            Protocol::Rtsp => "rtsp",
            Protocol::Mqtt => "mqtt",
            Protocol::Stomp => "stomp", // not officially supported
            Protocol::Smpp => "smpp",
            Protocol::RtmpOverTcp => "rtmpt.tcp",
            Protocol::RtmpOverHttp => "rtmpt.http",
            Protocol::Nats => "nats", // not officially supported
            Protocol::BitTorrentOverTcp => "bittorrent.tcp",
            Protocol::BitTorrentOverUtp => "bittorrent.utp",
            Protocol::Websocket => "websocket",
            Protocol::Dns => "dns",
        }
    }

    pub const fn wireshark_protocol(&self) -> &'static str {
        match self {
            Protocol::Unknown => "",
            Protocol::SslLegacy | Protocol::TlsLegacy | Protocol::TlsModern => "tls",
            Protocol::TlsTlcp => "tls",
            Protocol::Http1 => "http",
            Protocol::Http2 => "http2",
            Protocol::Http3 => "http3",
            Protocol::Smtp => "smtp",
            Protocol::SshLegacy | Protocol::Ssh => "ssh",
            Protocol::FtpControl => "ftp",
            Protocol::Pop3 => "pop",
            Protocol::Nntp | Protocol::Nnsp => "nntp",
            Protocol::Imap => "imap",
            Protocol::Rtsp => "rtsp",
            Protocol::Mqtt => "mqtt",
            Protocol::Stomp => "stomp", // not officially supported
            Protocol::Smpp => "smpp",
            Protocol::RtmpOverTcp | Protocol::RtmpOverHttp => "rtmpt",
            Protocol::Nats => "nats", // not officially supported
            Protocol::BitTorrentOverTcp | Protocol::BitTorrentOverUtp => "bittorrent",
            Protocol::Websocket => "websocket",
            Protocol::Dns => "dns",
        }
    }
}

impl From<AlpnProtocol> for Protocol {
    fn from(p: AlpnProtocol) -> Self {
        match p {
            AlpnProtocol::Http10 | AlpnProtocol::Http11 => Protocol::Http1,
            AlpnProtocol::Http2 => Protocol::Http2,
            AlpnProtocol::Http3 => Protocol::Http3,
            AlpnProtocol::Ftp => Protocol::FtpControl,
            AlpnProtocol::Imap => Protocol::Imap,
            AlpnProtocol::Pop3 => Protocol::Pop3,
            AlpnProtocol::Nntp => Protocol::Nntp,
            AlpnProtocol::Nnsp => Protocol::Nnsp,
            AlpnProtocol::Mqtt => Protocol::Mqtt,
            AlpnProtocol::DnsOverTls => Protocol::Dns,
            AlpnProtocol::DnsOverQuic => Protocol::Dns,
        }
    }
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

mod bittorrent;
mod dns;
mod ftp;
mod http;
mod imap;
mod mqtt;
mod nats;
mod nntp;
mod pop3;
mod rtmp;
mod rtsp;
mod smpp;
mod smtp;
mod ssh;
mod ssl;
mod stomp;
