/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::os::fd::RawFd;
use std::ptr;

use libc::c_int;
use openssl::error::ErrorStack;
use openssl::foreign_types::foreign_type;

use crate::ffi;

foreign_type! {
    ///
    type CType = ffi::ASYNC_WAIT_CTX;
    fn drop = ffi::ASYNC_WAIT_CTX_free;

    pub struct AsyncWaitCtx;
    pub struct AsyncWaitCtxRef;
}

impl AsyncWaitCtx {
    pub(super) fn new() -> Result<Self, ErrorStack> {
        let wait_ctx = unsafe { ffi::ASYNC_WAIT_CTX_new() };
        if wait_ctx.is_null() {
            Err(ErrorStack::get())
        } else {
            Ok(AsyncWaitCtx(wait_ctx))
        }
    }

    pub fn get_all_fds(&self) -> Result<Vec<RawFd>, ErrorStack> {
        let mut fd_count = 0usize;
        let r = unsafe {
            ffi::ASYNC_WAIT_CTX_get_all_fds(self.0, ptr::null_mut(), &mut fd_count as *mut usize)
        };
        if r != 1 {
            return Err(ErrorStack::get());
        }

        let mut fds: Vec<c_int> = vec![0; fd_count];
        let r = unsafe {
            ffi::ASYNC_WAIT_CTX_get_all_fds(self.0, fds.as_mut_ptr(), &mut fd_count as *mut usize)
        };
        if r != 1 {
            return Err(ErrorStack::get());
        }

        Ok(fds.into_iter().collect())
    }

    pub fn get_changed_fds(&self) -> Result<(Vec<RawFd>, Vec<RawFd>), ErrorStack> {
        let mut add_fd_count = 0usize;
        let mut del_fd_count = 0usize;
        let r = unsafe {
            ffi::ASYNC_WAIT_CTX_get_changed_fds(
                self.0,
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
            ffi::ASYNC_WAIT_CTX_get_changed_fds(
                self.0,
                add_fds.as_mut_ptr(),
                &mut add_fd_count as *mut usize,
                del_fds.as_mut_ptr(),
                &mut del_fd_count as *mut usize,
            )
        };
        if r != 1 {
            return Err(ErrorStack::get());
        }

        Ok((add_fds.into_iter().collect(), del_fds.into_iter().collect()))
    }
}

#[cfg(ossl300)]
mod ossl3 {
    use std::sync::Arc;

    use atomic_waker::AtomicWaker;
    use libc::{c_int, c_void};
    use openssl::error::ErrorStack;

    use super::{AsyncWaitCtx, ffi};

    impl AsyncWaitCtx {
        pub fn set_callback(&self, waker: &Arc<AtomicWaker>) -> Result<(), ErrorStack> {
            let r = unsafe {
                ffi::ASYNC_WAIT_CTX_set_callback(
                    self.0,
                    Some(wake),
                    Arc::as_ptr(waker) as *mut c_void,
                )
            };
            if r != 1 {
                Err(ErrorStack::get())
            } else {
                Ok(())
            }
        }

        pub fn get_callback_status(&self) -> c_int {
            unsafe { ffi::ASYNC_WAIT_CTX_get_status(self.0) }
        }
    }

    extern "C" fn wake(arg: *mut c_void) -> c_int {
        let waker = unsafe { &*(arg as *const AtomicWaker) };
        waker.wake();
        0
    }
}
