/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

#[derive(Clone, Copy, Debug, Hash, PartialEq, PartialOrd, Ord, Eq)]
pub enum ProxyRequestType {
    HttpForward,
    HttpsForward,
    FtpOverHttp,
    HttpConnect,
    SocksTcpConnect,
    SocksUdpAssociate,
}

impl FromStr for ProxyRequestType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "httpforward" | "http_forward" => Ok(ProxyRequestType::HttpForward),
            "httpsforward" | "https_forward" => Ok(ProxyRequestType::HttpsForward),
            "ftpoverhttp" | "ftp_over_http" => Ok(ProxyRequestType::FtpOverHttp),
            "httpconnect" | "http_connect" => Ok(ProxyRequestType::HttpConnect),
            "sockstcpconnect" | "socks_tcp_connect" => Ok(ProxyRequestType::SocksTcpConnect),
            "socksudpassociate" | "socks_udp_associate" => Ok(ProxyRequestType::SocksUdpAssociate),
            _ => Err(()),
        }
    }
}
