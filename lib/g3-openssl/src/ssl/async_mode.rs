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

use std::os::fd::RawFd;
#[cfg(ossl300)]
use std::sync::Arc;
use std::task::{Context, Poll};
use std::{io, ptr};

#[cfg(ossl300)]
use atomic_waker::AtomicWaker;
use libc::c_int;
#[cfg(ossl300)]
use libc::c_void;
use openssl::error::ErrorStack;
use openssl::foreign_types::ForeignTypeRef;
use openssl::ssl::{SslMode, SslRef};
#[cfg(ossl300)]
use openssl_sys::SSL;
use tokio::io::unix::AsyncFd;
use tokio::io::Interest;

use crate::ffi;

pub trait SslAsyncModeExt {
    fn is_async(&self) -> bool;
    fn waiting_for_async(&self) -> bool;
    #[cfg(ossl300)]
    fn async_status(&self) -> c_int;
    #[cfg(ossl300)]
    fn set_async_engine_waker(&self, waker: &Arc<AtomicWaker>) -> Result<(), ErrorStack>;
    fn get_changed_fds(&self) -> Result<(Vec<RawFd>, Vec<RawFd>), ErrorStack>;
}

impl SslAsyncModeExt for SslRef {
    fn is_async(&self) -> bool {
        (self.mode() & SslMode::ASYNC) == SslMode::ASYNC
    }

    fn waiting_for_async(&self) -> bool {
        unsafe { ffi::SSL_waiting_for_async(self.as_ptr()) == 1 }
    }

    #[cfg(ossl300)]
    fn async_status(&self) -> c_int {
        unsafe { ffi::SSL_get_async_status(self.as_ptr()) }
    }

    #[cfg(ossl300)]
    fn set_async_engine_waker(&self, waker: &Arc<AtomicWaker>) -> Result<(), ErrorStack> {
        let r = unsafe { ffi::SSL_set_async_callback(self.as_ptr(), Some(async_engine_wake)) };
        if r != 1 {
            return Err(ErrorStack::get());
        }

        let r = unsafe {
            ffi::SSL_set_async_callback_arg(self.as_ptr(), Arc::as_ptr(waker) as *mut c_void)
        };
        if r != 1 {
            Err(ErrorStack::get())
        } else {
            Ok(())
        }
    }

    fn get_changed_fds(&self) -> Result<(Vec<RawFd>, Vec<RawFd>), ErrorStack> {
        let mut add_fd_count = 0usize;
        let mut del_fd_count = 0usize;
        let r = unsafe {
            ffi::SSL_get_changed_async_fds(
                self.as_ptr(),
                ptr::null_mut(),
                &mut add_fd_count as *mut usize,
                ptr::null_mut(),
                &mut del_fd_count as *mut usize,
            )
        };
        if r != 1 {
            return Err(ErrorStack::get());
        }

        let mut add_fds: Vec<c_int> = vec![0; add_fd_count];
        let mut del_fds: Vec<c_int> = vec![0; del_fd_count];
        let r = unsafe {
            ffi::SSL_get_changed_async_fds(
                self.as_ptr(),
                add_fds.as_mut_ptr(),
                &mut add_fd_count as *mut usize,
                del_fds.as_mut_ptr(),
                &mut del_fd_count as *mut usize,
            )
        };
        if r != 1 {
            return Err(ErrorStack::get());
        }

        Ok((
            add_fds.into_iter().map(RawFd::from).collect(),
            del_fds.into_iter().map(RawFd::from).collect(),
        ))
    }
}

pub(crate) struct AsyncEnginePoller {
    tracked_fds: Vec<AsyncFd<RawFd>>,
    #[cfg(ossl300)]
    atomic_waker: Arc<AtomicWaker>,
}

impl AsyncEnginePoller {
    #[cfg(not(ossl300))]
    pub(crate) fn new(ssl: &SslRef) -> Option<Self> {
        if ssl.is_async() {
            Some(AsyncEnginePoller {
                tracked_fds: Vec::with_capacity(1),
            })
        } else {
            None
        }
    }

    #[cfg(ossl300)]
    pub(crate) fn new(ssl: &SslRef) -> Result<Option<Self>, ErrorStack> {
        if !ssl.is_async() {
            return Ok(None);
        }

        let atomic_waker = Arc::new(AtomicWaker::new());
        ssl.set_async_engine_waker(&atomic_waker)?;

        Ok(Some(AsyncEnginePoller {
            tracked_fds: Vec::with_capacity(1),
            atomic_waker,
        }))
    }

    #[cfg(ossl300)]
    pub(crate) fn set_cx(&self, cx: &mut Context<'_>) {
        self.atomic_waker.register(cx.waker());
    }

    #[cfg(not(ossl300))]
    pub(crate) fn poll_ready(
        &mut self,
        ssl: &SslRef,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        let (add, del) = ssl.get_changed_fds().map_err(io::Error::other)?;
        for fd in add {
            let async_fd = AsyncFd::with_interest(fd, Interest::READABLE)?;
            self.tracked_fds.push(async_fd);
        }
        for fd in del {
            self.tracked_fds.retain(|v| fd.ne(v.get_ref()));
        }

        for fd in &self.tracked_fds {
            match fd.poll_read_ready(cx) {
                Poll::Pending => {}
                Poll::Ready(Ok(_)) => return Poll::Ready(Ok(())),
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            }
        }
        Poll::Pending
    }

    #[cfg(ossl300)]
    pub(crate) fn poll_ready(
        &mut self,
        ssl: &SslRef,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        match ssl.async_status() {
            ffi::ASYNC_STATUS_UNSUPPORTED => {
                let (add, del) = ssl.get_changed_fds().map_err(io::Error::other)?;
                for fd in add {
                    let async_fd = AsyncFd::with_interest(fd, Interest::READABLE)?;
                    self.tracked_fds.push(async_fd);
                }
                for fd in del {
                    self.tracked_fds.retain(|v| fd.ne(v.get_ref()));
                }

                for fd in &self.tracked_fds {
                    match fd.poll_read_ready(cx) {
                        Poll::Pending => {}
                        Poll::Ready(Ok(_)) => return Poll::Ready(Ok(())),
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    }
                }
                Poll::Pending
            }
            ffi::ASYNC_STATUS_ERR => Poll::Ready(Err(io::Error::other(ErrorStack::get()))),
            ffi::ASYNC_STATUS_OK => {
                // submitted, wait for the callback
                Poll::Pending
            }
            ffi::ASYNC_STATUS_EAGAIN => {
                // engine busy, resume later
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            r => Poll::Ready(Err(io::Error::other(format!(
                "SSL_get_async_status returned {r}"
            )))),
        }
    }
}

#[cfg(ossl300)]
extern "C" fn async_engine_wake(_ssl: *mut SSL, arg: *mut c_void) -> c_int {
    let waker = unsafe { &*(arg as *const AtomicWaker) };
    waker.wake();
    0
}
