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

use std::ffi::CStr;
use std::fs::File;
use std::io::Write;
use std::os::fd::AsFd;
use std::os::unix::net::UnixDatagram;

use anyhow::{anyhow, Context};
use once_cell::sync::OnceCell;
use rustix::cmsg_space;
use rustix::fs::{fcntl_add_seals, memfd_create, MemfdFlags, SealFlags};
use rustix::io::Errno;
use rustix::net::{
    sendmsg_unix, SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketAddrUnix,
};

/// Default path of the systemd-journald `AF_UNIX` datagram socket.
const SD_JOURNAL_SOCK_PATH: &str = "/run/systemd/journal/socket";
/// The name is used as a filename in /proc/self/fd/, always prefixed with memfd.
/// Multiple memfd files can have the same name without any side effects.
const MEM_FD_NAME: &CStr = unsafe { CStr::from_bytes_with_nul_unchecked(b"journal-logging\0") };

thread_local! {
    static SD_SOCK: OnceCell<UnixDatagram> = const { OnceCell::new() };
}

#[allow(unused)]
pub(crate) fn journal_send(data: &[u8]) -> anyhow::Result<()> {
    SD_SOCK.with(|cell| {
        cell.get_or_try_init(|| {
            UnixDatagram::unbound()
                .map_err(|e| anyhow!("failed to create unbounded unix socket: {e}"))
        })
        .and_then(|sock| send_payload(sock, data))
    })
}

fn send_payload(sock: &UnixDatagram, data: &[u8]) -> anyhow::Result<()> {
    if let Err(e) = sock.send_to(data, SD_JOURNAL_SOCK_PATH) {
        if e.raw_os_error() == Some(Errno::MSGSIZE.raw_os_error()) {
            // fallback if size limit reached
            send_memfd_payload(sock, data).context("sending with memfd failed")
        } else {
            Err(anyhow!("send_to failed: {e}"))
        }
    } else {
        Ok(())
    }
}

/// Send an overlarge payload to systemd-journald socket.
///
/// This is a slow-path for sending a large payload that could not otherwise fit
/// in a UNIX datagram. Payload is thus written to a memfd, which is sent as ancillary
/// data.
fn send_memfd_payload(sock: &UnixDatagram, data: &[u8]) -> anyhow::Result<()> {
    let tmpfd = memfd_create(MEM_FD_NAME, MemfdFlags::ALLOW_SEALING)
        .map_err(|e| anyhow!("unable to create memfd: {e}"))?;

    let mut mem_file = File::from(tmpfd);
    mem_file
        .write_all(data)
        .map_err(|e| anyhow!("failed to write to memfd: {e}"))?;

    // Seal the memfd, so that journald knows it can safely mmap/read it.
    fcntl_add_seals(mem_file.as_fd(), SealFlags::all())
        .map_err(|e| anyhow!("unable to seal memfd: {e}"))?;

    let fds = &[mem_file.as_fd()];
    let mut space = [0; cmsg_space!(ScmRights(1))];
    let mut control = SendAncillaryBuffer::new(&mut space);
    control.push(SendAncillaryMessage::ScmRights(fds));
    let addr = SocketAddrUnix::new(SD_JOURNAL_SOCK_PATH)
        .map_err(|e| anyhow!("unable to create new unix address: {e}"))?;
    sendmsg_unix(sock.as_fd(), &addr, &[], &mut control, SendFlags::empty())
        .map_err(|e| anyhow!("sendmsg failed: {e}"))?;

    // Close our side of the memfd after we send it to systemd.
    drop(mem_file);

    Ok(())
}
