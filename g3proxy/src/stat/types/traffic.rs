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

use g3_types::stats::{TcpIoSnapshot, TcpIoStats, UdpIoSnapshot, UdpIoStats};

#[derive(Default)]
pub(crate) struct TrafficStats {
    pub(crate) http_forward: TcpIoStats,
    pub(crate) https_forward: TcpIoStats,
    pub(crate) http_connect: TcpIoStats,
    pub(crate) ftp_over_http: TcpIoStats,
    pub(crate) socks_tcp_connect: TcpIoStats,
    pub(crate) socks_udp_connect: UdpIoStats,
    pub(crate) socks_udp_associate: UdpIoStats,
}

#[derive(Default)]
pub(crate) struct TrafficSnapshot {
    pub(crate) http_forward: TcpIoSnapshot,
    pub(crate) https_forward: TcpIoSnapshot,
    pub(crate) http_connect: TcpIoSnapshot,
    pub(crate) ftp_over_http: TcpIoSnapshot,
    pub(crate) socks_tcp_connect: TcpIoSnapshot,
    pub(crate) socks_udp_connect: UdpIoSnapshot,
    pub(crate) socks_udp_associate: UdpIoSnapshot,
}

#[derive(Default)]
pub(crate) struct UpstreamTrafficStats {
    pub(crate) tcp: TcpIoStats,
    pub(crate) udp: UdpIoStats,
}

#[derive(Default)]
pub(crate) struct UpstreamTrafficSnapshot {
    pub(crate) tcp: TcpIoSnapshot,
    pub(crate) udp: UdpIoSnapshot,
}
