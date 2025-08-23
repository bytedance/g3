/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::cell::RefCell;

use crate::udp::{RecvAncillaryBuffer, RecvMsgHdr};

thread_local! {
    static RECV_ANCILLARY_BUFFERS: RefCell<Vec<RecvAncillaryBuffer>> = const { RefCell::new(Vec::new()) };
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
    ))]
    static RECVMMSG_BUF: RefCell<Vec<libc::mmsghdr>> = const { RefCell::new(Vec::new()) };
    #[cfg(target_os = "macos")]
    static RECVMSG_X_BUF: RefCell<Vec<crate::ffi::msghdr_x>> = const { RefCell::new(Vec::new()) };
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "solaris",
))]
pub fn with_recvmmsg_buf<const C: usize, F, R>(hdr_v: &mut [RecvMsgHdr<'_, C>], mut run: F) -> R
where
    F: FnMut(&mut [RecvMsgHdr<'_, C>], &mut [libc::mmsghdr]) -> R,
{
    RECV_ANCILLARY_BUFFERS.with_borrow_mut(|buffers| {
        if buffers.len() < hdr_v.len() {
            buffers.resize_with(hdr_v.len(), RecvAncillaryBuffer::default);
        }

        RECVMMSG_BUF.with_borrow_mut(|msgvec| {
            msgvec.clear();
            msgvec.reserve(hdr_v.len());

            for (i, m) in hdr_v.iter_mut().enumerate() {
                let control_buf = &mut buffers[i];
                msgvec.push(unsafe { m.to_mmsghdr(control_buf) });
            }

            run(hdr_v, msgvec)
        })
    })
}

#[cfg(target_os = "macos")]
pub fn with_recvmsg_x_buf<const C: usize, F, R>(hdr_v: &mut [RecvMsgHdr<'_, C>], mut run: F) -> R
where
    F: FnMut(&mut [RecvMsgHdr<'_, C>], &mut [crate::ffi::msghdr_x]) -> R,
{
    RECV_ANCILLARY_BUFFERS.with_borrow_mut(|buffers| {
        if buffers.len() < hdr_v.len() {
            buffers.resize_with(hdr_v.len(), RecvAncillaryBuffer::default);
        }

        RECVMSG_X_BUF.with_borrow_mut(|msgvec| {
            msgvec.clear();
            msgvec.reserve(hdr_v.len());

            for (i, m) in hdr_v.iter_mut().enumerate() {
                let control_buf = &mut buffers[i];
                msgvec.push(unsafe { m.to_msghdr_x(control_buf) });
            }

            run(hdr_v, msgvec)
        })
    })
}
