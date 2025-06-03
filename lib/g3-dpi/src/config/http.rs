/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::num::NonZeroUsize;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct H1InterceptionConfig {
    pub pipeline_size: NonZeroUsize,
    pub pipeline_read_idle_timeout: Duration,
    pub req_head_recv_timeout: Duration,
    pub rsp_head_recv_timeout: Duration,
    pub req_head_max_size: usize,
    pub rsp_head_max_size: usize,
    pub body_line_max_len: usize,
    pub steal_forwarded_for: bool,
}

impl Default for H1InterceptionConfig {
    fn default() -> Self {
        H1InterceptionConfig {
            pipeline_size: NonZeroUsize::new(10).unwrap(),
            pipeline_read_idle_timeout: Duration::from_secs(300),
            req_head_recv_timeout: Duration::from_secs(30),
            rsp_head_recv_timeout: Duration::from_secs(60),
            req_head_max_size: 65536,
            rsp_head_max_size: 65536,
            body_line_max_len: 8192,
            steal_forwarded_for: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct H2InterceptionConfig {
    pub max_header_list_size: u32,
    pub max_concurrent_streams: u32,
    max_frame_size: u32,
    pub max_send_buffer_size: usize,
    pub upstream_handshake_timeout: Duration,
    pub upstream_stream_open_timeout: Duration,
    pub client_handshake_timeout: Duration,
    pub ping_interval: Duration,
    pub rsp_head_recv_timeout: Duration,
    pub silent_drop_expect_header: bool,
}

impl Default for H2InterceptionConfig {
    fn default() -> Self {
        H2InterceptionConfig {
            max_header_list_size: 64 * 1024, // 64KB
            max_concurrent_streams: 16,
            max_frame_size: 1024 * 1024,            // 1MB
            max_send_buffer_size: 16 * 1024 * 1024, // 16MB
            upstream_handshake_timeout: Duration::from_secs(10),
            upstream_stream_open_timeout: Duration::from_secs(10),
            client_handshake_timeout: Duration::from_secs(4),
            ping_interval: Duration::from_secs(60),
            rsp_head_recv_timeout: Duration::from_secs(60),
            silent_drop_expect_header: false,
        }
    }
}

impl H2InterceptionConfig {
    #[inline]
    pub fn max_frame_size(&self) -> u32 {
        self.max_frame_size
    }

    pub fn set_max_frame_size(&mut self, size: u32) {
        self.max_frame_size = size.clamp(1 << 14, (1 << 24) - 1);
    }
}
