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
use std::ptr;
#[cfg(feature = "callback")]
use std::task::Waker;

use foreign_types::foreign_type;
use libc::c_int;
#[cfg(feature = "callback")]
use libc::c_void;
use openssl::error::ErrorStack;

use super::ffi;

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

    #[cfg(feature = "callback")]
    pub fn set_callback(&self, waker: &Waker) -> Result<(), ErrorStack> {
        let r = unsafe {
            ffi::ASYNC_WAIT_CTX_set_callback(self.0, Some(wake), waker as *const _ as *mut c_void)
        };
        if r != 1 {
            Err(ErrorStack::get())
        } else {
            Ok(())
        }
    }

    #[cfg(feature = "callback")]
    pub fn get_callback_status(&self) -> c_int {
        unsafe { ffi::ASYNC_WAIT_CTX_get_status(self.0) }
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

        Ok(fds.into_iter().map(RawFd::from).collect())
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

        Ok((
            add_fds.into_iter().map(RawFd::from).collect(),
            del_fds.into_iter().map(RawFd::from).collect(),
        ))
    }
}

#[cfg(feature = "callback")]
extern "C" fn wake(arg: *mut c_void) -> c_int {
    let ptr = ptr::NonNull::new(arg as *mut Waker).unwrap();
    let waker = unsafe { ptr.as_ref() };
    waker.wake_by_ref();
    0
}
