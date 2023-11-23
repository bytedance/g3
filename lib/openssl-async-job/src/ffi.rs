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

#![allow(unused)]

use libc::{c_int, c_void};

#[allow(non_camel_case_types)]
pub enum ASYNC_JOB {}

#[allow(non_camel_case_types)]
pub enum ASYNC_WAIT_CTX {}

#[allow(non_camel_case_types)]
pub type ASYNC_callback_fn = Option<unsafe extern "C" fn(arg: *mut c_void) -> c_int>;

pub const ASYNC_ERR: c_int = 0;
pub const ASYNC_NO_JOBS: c_int = 1;
pub const ASYNC_PAUSE: c_int = 2;
pub const ASYNC_FINISH: c_int = 3;

pub const ASYNC_STATUS_UNSUPPORTED: c_int = 0;
pub const ASYNC_STATUS_ERR: c_int = 1;
pub const ASYNC_STATUS_OK: c_int = 2;
pub const ASYNC_STATUS_EAGAIN: c_int = 3;

extern "C" {
    pub fn ASYNC_is_capable() -> c_int;
    pub fn ASYNC_init_thread(max_size: usize, init_size: usize) -> c_int;
    pub fn ASYNC_cleanup_thread();

    pub fn ASYNC_start_job(
        job: *mut *mut ASYNC_JOB,
        ctx: *mut ASYNC_WAIT_CTX,
        ret: *mut c_int,
        func: Option<unsafe extern "C" fn(arg1: *mut c_void) -> c_int>,
        args: *mut c_void,
        size: usize,
    ) -> c_int;
    pub fn ASYNC_pause_job() -> c_int;
    pub fn ASYNC_get_current_job() -> *mut ASYNC_JOB;
    pub fn ASYNC_get_wait_ctx(job: *mut ASYNC_JOB) -> *mut ASYNC_WAIT_CTX;
    pub fn ASYNC_block_pause();
    pub fn ASYNC_unblock_pause();

    pub fn ASYNC_WAIT_CTX_new() -> *mut ASYNC_WAIT_CTX;
    pub fn ASYNC_WAIT_CTX_free(ctx: *mut ASYNC_WAIT_CTX);
    pub fn ASYNC_WAIT_CTX_set_wait_fd(
        ctx: *mut ASYNC_WAIT_CTX,
        key: *const c_void,
        fd: c_int,
        custom_data: *mut c_void,
        cleanup: Option<
            unsafe extern "C" fn(
                arg1: *mut ASYNC_WAIT_CTX,
                arg2: *const c_void,
                arg3: c_int,
                arg4: *mut c_void,
            ),
        >,
    ) -> c_int;
    pub fn ASYNC_WAIT_CTX_get_fd(
        ctx: *mut ASYNC_WAIT_CTX,
        key: *const c_void,
        fd: *mut c_int,
        custom_data: *mut *mut c_void,
    ) -> c_int;
    pub fn ASYNC_WAIT_CTX_get_all_fds(
        ctx: *mut ASYNC_WAIT_CTX,
        fd: *mut c_int,
        numfds: *mut usize,
    ) -> c_int;
    pub fn ASYNC_WAIT_CTX_get_changed_fds(
        ctx: *mut ASYNC_WAIT_CTX,
        addfd: *mut c_int,
        numaddfds: *mut usize,
        delfd: *mut c_int,
        numdelfds: *mut usize,
    ) -> c_int;
    pub fn ASYNC_WAIT_CTX_clear_fd(ctx: *mut ASYNC_WAIT_CTX, key: *const c_void) -> c_int;
    pub fn ASYNC_WAIT_CTX_get_callback(
        ctx: *mut ASYNC_WAIT_CTX,
        callback: *mut ASYNC_callback_fn,
        callback_arg: *mut *mut c_void,
    ) -> c_int;
    pub fn ASYNC_WAIT_CTX_set_callback(
        ctx: *mut ASYNC_WAIT_CTX,
        callback: ASYNC_callback_fn,
        callback_arg: *mut c_void,
    ) -> c_int;
    pub fn ASYNC_WAIT_CTX_set_status(ctx: *mut ASYNC_WAIT_CTX, status: c_int) -> c_int;
    pub fn ASYNC_WAIT_CTX_get_status(ctx: *mut ASYNC_WAIT_CTX) -> c_int;
}
