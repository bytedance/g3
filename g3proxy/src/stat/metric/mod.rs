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

pub(super) mod escaper;
pub(super) mod resolver;
pub(super) mod server;

pub(super) mod user;
use user::{RequestStatsNamesRef, TrafficStatsNamesRef};

pub(crate) mod user_site;

const TAG_KEY_SERVER: &str = "server";
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
