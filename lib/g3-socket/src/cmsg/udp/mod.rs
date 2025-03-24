/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use std::io;
use std::net::IpAddr;
use std::time::Duration;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

const CMSG_RECV_BUFFER_SIZE: usize = 10240; // see rfc3542 20.1

pub trait RecvAncillaryData {
    fn set_recv_interface(&mut self, id: u32);
    fn set_recv_dst_addr(&mut self, addr: IpAddr);
    fn set_timestamp(&mut self, ts: Duration);
}

pub struct RecvAncillaryBuffer {
    buf: [u8; CMSG_RECV_BUFFER_SIZE],
}

impl Default for RecvAncillaryBuffer {
    fn default() -> Self {
        RecvAncillaryBuffer {
            buf: [0u8; CMSG_RECV_BUFFER_SIZE],
        }
    }
}

impl RecvAncillaryBuffer {
    pub fn as_bytes(&self) -> &[u8] {
        self.buf.as_slice()
    }

    pub fn parse<T: RecvAncillaryData>(&self, total_size: usize, data: &mut T) -> io::Result<()> {
        Self::parse_buf(&self.buf[..total_size], data)
    }
}
