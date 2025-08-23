/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::net::UdpSocket;

use super::SendMsgHdr;

pub trait UdpSocketExt {
    fn sendmsg<const C: usize>(&self, hdr: &SendMsgHdr<'_, C>) -> io::Result<usize>;

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
    ))]
    fn batch_sendmsg<const C: usize>(&self, msgs: &mut [SendMsgHdr<'_, C>]) -> io::Result<usize>;
    #[cfg(target_os = "macos")]
    fn batch_sendmsg_x<const C: usize>(&self, msgs: &mut [SendMsgHdr<'_, C>]) -> io::Result<usize>;
}

impl UdpSocketExt for UdpSocket {
    fn sendmsg<const C: usize>(&self, hdr: &SendMsgHdr<'_, C>) -> io::Result<usize> {
        let mut msghdr = unsafe { hdr.to_msghdr() };
        super::sendmsg(self, &mut msghdr)
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
    ))]
    fn batch_sendmsg<const C: usize>(&self, msgs: &mut [SendMsgHdr<'_, C>]) -> io::Result<usize> {
        crate::udp::with_sendmmsg_buf(msgs, |msgs, msgvec| {
            let count = super::sendmmsg(self, msgvec)?;
            for (m, h) in msgs.iter_mut().take(count).zip(msgvec) {
                m.n_send = h.msg_len as usize;
            }
            Ok(count)
        })
    }

    #[cfg(target_os = "macos")]
    fn batch_sendmsg_x<const C: usize>(&self, msgs: &mut [SendMsgHdr<'_, C>]) -> io::Result<usize> {
        crate::udp::with_sendmsg_x_buf(msgs, |msgs, msgvec| {
            let count = super::sendmsg_x(self, msgvec)?;
            for m in msgs.iter_mut().take(count) {
                m.n_send = m.iov.iter().map(|iov| iov.len()).sum();
            }
            Ok(count)
        })
    }
}
