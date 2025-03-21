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

use super::{CMSG_RECV_BUFFER_SIZE, RecvAncillaryData};

pub struct RecvAncillaryBuffer {
    buf: [u8; CMSG_RECV_BUFFER_SIZE],
}

impl RecvAncillaryBuffer {
    pub fn as_bytes(&self) -> &[u8] {
        self.buf.as_slice()
    }

    pub fn parse<T: RecvAncillaryData>(&self, total_size: usize, data: &mut T) -> io::Result<()> {
        let mut offset = 0usize;

        while offset + size_of::<libc::cmsghdr>() <= total_size {
            let buf = &self.buf[offset..];
            let hdr = unsafe { (buf.as_ptr() as *const libc::cmsghdr).as_ref().unwrap() };
            offset += hdr.cmsg_len;

            match hdr.cmsg_level {
                libc::SOL_SOCKET => {}
                libc::IPPROTO_IP => {}
                libc::IPPROTO_IPV6 => match hdr.cmsg_type {
                    libc::IPV6_PKTINFO => {}
                    _ => {}
                },
                _ => {}
            }
        }

        Ok(())
    }
}
