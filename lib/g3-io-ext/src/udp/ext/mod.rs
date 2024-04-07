/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

#[cfg(unix)]
use std::io::IoSliceMut;
use std::io::{self, IoSlice};
use std::net::SocketAddr;
use std::task::{Context, Poll};

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::{RecvMsgHdr, SendMsgHdr};

#[cfg(windows)]
mod windows;

pub trait UdpSocketExt {
    fn poll_sendmsg(
        &self,
        cx: &mut Context<'_>,
        iov: &[IoSlice<'_>],
        target: Option<SocketAddr>,
    ) -> Poll<io::Result<usize>>;

    fn try_sendmsg(&self, iov: &[IoSlice<'_>], target: Option<SocketAddr>) -> io::Result<usize>;

    #[cfg(unix)]
    fn poll_recvmsg(
        &self,
        cx: &mut Context<'_>,
        iov: &mut [IoSliceMut<'_>],
    ) -> Poll<io::Result<(usize, Option<SocketAddr>)>>;

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
    ))]
    fn poll_batch_sendmsg<const C: usize>(
        &self,
        cx: &mut Context<'_>,
        msgs: &mut [SendMsgHdr<'_, C>],
    ) -> Poll<io::Result<usize>>;

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
    ))]
    fn poll_batch_recvmsg<const C: usize>(
        &self,
        cx: &mut Context<'_>,
        hdr_v: &mut [RecvMsgHdr<'_, C>],
    ) -> Poll<io::Result<usize>>;
}
