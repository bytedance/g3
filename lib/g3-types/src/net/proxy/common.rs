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
