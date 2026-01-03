/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_types::stats::{TcpIoSnapshot, TcpIoStats, UdpIoSnapshot, UdpIoStats};

#[derive(Default)]
pub(crate) struct TrafficStats {
    pub(crate) tcp_connect: TcpIoStats,
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
    pub(crate) tcp_connect: TcpIoSnapshot,
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
