/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

pub(super) mod escaper;
pub(super) mod resolver;
pub(super) mod server;

pub(super) mod user;
use user::{RequestStatsNamesRef, TrafficStatsNamesRef, UserMetricExt};

pub(crate) mod user_site;

const TAG_KEY_ESCAPER: &str = "escaper";

#[derive(Copy, Clone)]
enum MetricUserConnectionType {
    Http,
    Socks,
}

impl MetricUserConnectionType {
    const fn as_str(&self) -> &'static str {
        match self {
            MetricUserConnectionType::Http => "http",
            MetricUserConnectionType::Socks => "socks",
        }
    }
}

impl AsRef<str> for MetricUserConnectionType {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Copy, Clone)]
enum MetricUserRequestType {
    HttpForward,
    HttpsForward,
    HttpConnect,
    FtpOverHttp,
    SocksTcpConnect,
    SocksUdpConnect,
    SocksUdpAssociate,
}

impl MetricUserRequestType {
    const fn as_str(&self) -> &'static str {
        match self {
            MetricUserRequestType::HttpForward => "http_forward",
            MetricUserRequestType::HttpsForward => "https_forward",
            MetricUserRequestType::HttpConnect => "http_connect",
            MetricUserRequestType::FtpOverHttp => "ftp_over_http",
            MetricUserRequestType::SocksTcpConnect => "socks_tcp_connect",
            MetricUserRequestType::SocksUdpConnect => "socks_udp_connect",
            MetricUserRequestType::SocksUdpAssociate => "socks_udp_associate",
        }
    }
}

impl AsRef<str> for MetricUserRequestType {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
