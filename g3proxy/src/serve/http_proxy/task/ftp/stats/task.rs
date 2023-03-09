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

use crate::module::ftp_over_http::{FtpTaskRemoteControlStats, FtpTaskRemoteTransferStats};

#[derive(Default)]
pub(crate) struct TcpHalfConnectionStats {
    bytes: u64,
}

impl TcpHalfConnectionStats {
    pub(crate) fn get_bytes(&self) -> u64 {
        self.bytes
    }

    pub(crate) fn add_bytes(&self, size: u64) {
        unsafe {
            let r = &self.bytes as *const u64 as *mut u64;
            *r += size;
        }
    }
}

#[derive(Default)]
pub(crate) struct FtpOverHttpClientStats {
    pub(crate) read: TcpHalfConnectionStats,
    pub(crate) write: TcpHalfConnectionStats,
}

#[derive(Default)]
pub(crate) struct FtpOverHttpServerStats {
    pub(crate) control_read: TcpHalfConnectionStats,
    pub(crate) control_write: TcpHalfConnectionStats,
    pub(crate) transfer_read: TcpHalfConnectionStats,
    pub(crate) transfer_write: TcpHalfConnectionStats,
}

#[derive(Default)]
pub(crate) struct FtpOverHttpTaskStats {
    pub(crate) http_client: FtpOverHttpClientStats,
    pub(crate) ftp_server: FtpOverHttpServerStats,
}

impl FtpTaskRemoteControlStats for FtpOverHttpTaskStats {
    fn add_read_bytes(&self, size: u64) {
        self.ftp_server.control_read.add_bytes(size);
    }

    fn add_write_bytes(&self, size: u64) {
        self.ftp_server.control_write.add_bytes(size);
    }
}

impl FtpTaskRemoteTransferStats for FtpOverHttpTaskStats {
    fn add_read_bytes(&self, size: u64) {
        self.ftp_server.transfer_read.add_bytes(size);
    }

    fn add_write_bytes(&self, size: u64) {
        self.ftp_server.transfer_write.add_bytes(size);
    }
}
