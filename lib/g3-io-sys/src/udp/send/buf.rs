/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::cell::RefCell;

use super::SendMsgHdr;

thread_local! {
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
    ))]
    static SENDMMSG_BUF: RefCell<Vec<libc::mmsghdr>> = const { RefCell::new(Vec::new()) };
    #[cfg(target_os = "macos")]
    static SENDMSG_X_BUF: RefCell<Vec<crate::ffi::msghdr_x>> = const { RefCell::new(Vec::new()) };
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "solaris",
))]
pub fn with_sendmmsg_buf<const C: usize, F, R>(msgs: &mut [SendMsgHdr<'_, C>], mut run: F) -> R
where
    F: FnMut(&mut [SendMsgHdr<'_, C>], &mut [libc::mmsghdr]) -> R,
{
    SENDMMSG_BUF.with_borrow_mut(|msgvec| {
        msgvec.clear();
        msgvec.reserve(msgs.len());

        for m in msgs.iter_mut() {
            msgvec.push(unsafe { m.to_mmsghdr() });
        }

        run(msgs, msgvec.as_mut_slice())
    })
}

#[cfg(target_os = "macos")]
pub fn with_sendmsg_x_buf<const C: usize, F, R>(msgs: &mut [SendMsgHdr<'_, C>], mut run: F) -> R
where
    F: FnMut(&mut [SendMsgHdr<'_, C>], &mut [crate::ffi::msghdr_x]) -> R,
{
    SENDMSG_X_BUF.with_borrow_mut(|msgvec| {
        msgvec.clear();
        msgvec.reserve(msgs.len());

        for m in msgs.iter_mut() {
            msgvec.push(unsafe { m.to_msghdr_x() });
        }

        run(msgs, msgvec.as_mut_slice())
    })
}
