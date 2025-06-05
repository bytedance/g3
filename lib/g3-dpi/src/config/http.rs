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

use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct H1InterceptionConfig {
    pub pipeline_size: usize,
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
            pipeline_size: 10,
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
    stream_window_size: u32,
    connection_window_size: u32,
    max_frame_size: u32,
    pub max_send_buffer_size: usize,
    pub upstream_handshake_timeout: Duration,
    pub upstream_stream_open_timeout: Duration,
    pub client_handshake_timeout: Duration,
    pub rsp_head_recv_timeout: Duration,
    pub silent_drop_expect_header: bool,
}

impl Default for H2InterceptionConfig {
    fn default() -> Self {
        H2InterceptionConfig {
            max_header_list_size: 64 * 1024, // 64KB
            max_concurrent_streams: 128,
            stream_window_size: 1024 * 1024,         // 1MiB
            connection_window_size: 2 * 1024 * 1024, // 2MiB
            max_frame_size: 256 * 1024,              // 256KiB
            max_send_buffer_size: 8 * 1024 * 1024,   // 8MiB
            upstream_handshake_timeout: Duration::from_secs(10),
            upstream_stream_open_timeout: Duration::from_secs(10),
            client_handshake_timeout: Duration::from_secs(4),
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

    #[inline]
    pub fn stream_window_size(&self) -> u32 {
        self.stream_window_size
    }

    pub fn set_stream_window_size(&mut self, size: u32) {
        self.stream_window_size = size.max(65536);
    }

    #[inline]
    pub fn connection_window_size(&self) -> u32 {
        self.connection_window_size
    }

    pub fn set_connection_window_size(&mut self, size: u32) {
        self.connection_window_size = size.max(65536);
    }
}
