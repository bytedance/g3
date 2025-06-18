/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::cell::RefCell;
use std::io;
use std::task::{Context, Poll, ready};

use tokio::io::Interest;
use tokio::net::UdpSocket;

use g3_io_sys::udp::{RecvAncillaryBuffer, RecvMsgHdr, SendMsgHdr, recvmsg, sendmsg};

use super::UdpSocketExt;

thread_local! {
    static RECV_ANCILLARY_BUFFER: RefCell<RecvAncillaryBuffer> = const { RefCell::new(RecvAncillaryBuffer::new()) };
}

impl UdpSocketExt for UdpSocket {
    fn poll_sendmsg<const C: usize>(
        &self,
        cx: &mut Context<'_>,
        hdr: &SendMsgHdr<'_, C>,
    ) -> Poll<io::Result<usize>> {
        let mut msghdr = unsafe { hdr.to_msghdr() };

        loop {
            ready!(self.poll_send_ready(cx))?;
            match self.try_io(Interest::WRITABLE, || sendmsg(self, &mut msghdr)) {
                Ok(res) => return Poll::Ready(Ok(res)),
                Err(e) => {
                    if e.kind() == io::ErrorKind::WouldBlock {
                        continue;
                    }
                    return Poll::Ready(Err(e));
                }
            }
        }
    }

    fn try_sendmsg<const C: usize>(&self, hdr: &SendMsgHdr<'_, C>) -> io::Result<usize> {
        let mut msghdr = unsafe { hdr.to_msghdr() };
        self.try_io(Interest::WRITABLE, || sendmsg(self, &mut msghdr))
    }

    fn poll_recvmsg<const C: usize>(
        &self,
        cx: &mut Context<'_>,
        hdr: &mut RecvMsgHdr<'_, C>,
    ) -> Poll<io::Result<()>> {
        RECV_ANCILLARY_BUFFER.with_borrow_mut(|control_buf| {
            let mut msghdr = unsafe { hdr.to_msghdr(control_buf) };
            loop {
                ready!(self.poll_recv_ready(cx))?;
                match self.try_io(Interest::READABLE, || recvmsg(self, &mut msghdr)) {
                    Ok(nr) => {
                        hdr.n_recv = nr;
                        control_buf.parse(msghdr.Control.len as _, hdr)?;
                        return Poll::Ready(Ok(()));
                    }
                    Err(e) => {
                        if e.kind() == io::ErrorKind::WouldBlock {
                            continue;
                        } else {
                            return Poll::Ready(Err(e));
                        }
                    }
                }
            }
        })
    }
}
