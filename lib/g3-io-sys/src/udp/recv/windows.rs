/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::os::windows::io::AsRawSocket;
use std::{io, ptr};

use once_cell::sync::OnceCell;
use windows_sys::Win32::Networking::WinSock;

use super::RecvMsgHdr;
use crate::udp::RecvAncillaryBuffer;

thread_local! {
    static WSARECVMSG_PTR: OnceCell<WinSock::LPFN_WSARECVMSG> = const { OnceCell::new() };
}

impl<const C: usize> RecvMsgHdr<'_, C> {
    /// # Safety
    ///
    /// `self` should not be dropped before the returned value
    pub unsafe fn to_msghdr(&self, control_buf: &mut RecvAncillaryBuffer) -> WinSock::WSAMSG {
        let control_buf = control_buf.as_bytes();
        unsafe {
            let c_addr = &mut *self.c_addr.get();
            let (name, namelen) = c_addr.get_ptr_and_size();

            WinSock::WSAMSG {
                name,
                namelen,
                lpBuffers: self.iov.as_ptr() as _,
                dwBufferCount: C as _,
                Control: WinSock::WSABUF {
                    len: control_buf.len() as _,
                    buf: control_buf.as_ptr() as _,
                },
                dwFlags: 0,
            }
        }
    }
}

pub fn recvmsg<T: AsRawSocket>(socket: &T, msghdr: &mut WinSock::WSAMSG) -> io::Result<usize> {
    WSARECVMSG_PTR.with(|v| {
        let wsa_recvmsg_ptr = v.get_or_try_init(|| get_wsa_recvmsg_ptr(socket))?;

        let Some(wsa_recvmsg_ptr) = wsa_recvmsg_ptr else {
            return Err(io::Error::other("WSARECVMSG function is not available"));
        };

        let mut n_recv = 0;
        let r = unsafe {
            (wsa_recvmsg_ptr)(
                socket.as_raw_socket() as _,
                ptr::from_mut(msghdr),
                ptr::from_mut(&mut n_recv),
                ptr::null_mut(),
                None,
            )
        };
        if r != 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(n_recv as usize)
        }
    })
}

fn get_wsa_recvmsg_ptr<T: AsRawSocket>(socket: &T) -> io::Result<WinSock::LPFN_WSARECVMSG> {
    let guid = WinSock::WSAID_WSARECVMSG;
    let mut wsa_recvmsg_ptr = None;
    let mut len = 0;

    // Safety: Option handles the NULL pointer with a None value
    let rc = unsafe {
        WinSock::WSAIoctl(
            socket.as_raw_socket() as _,
            WinSock::SIO_GET_EXTENSION_FUNCTION_POINTER,
            ptr::from_ref(&guid).cast(),
            size_of_val(&guid) as u32,
            ptr::from_mut(&mut wsa_recvmsg_ptr).cast(),
            size_of_val(&wsa_recvmsg_ptr) as u32,
            &mut len,
            ptr::null_mut(),
            None,
        )
    };
    if rc == -1 {
        return Err(io::Error::last_os_error());
    } else if len as usize != size_of::<WinSock::LPFN_WSARECVMSG>() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid WSARecvMsg function pointer: size mismatch",
        ));
    }

    Ok(wsa_recvmsg_ptr)
}
