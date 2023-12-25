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

use libc::{c_int, c_long, c_void};
use openssl_sys::{SSL_ctrl, SSL, SSL_CTRL_MODE, SSL_CTX};
use std::ptr;

pub const ASYNC_STATUS_UNSUPPORTED: c_int = 0;
pub const ASYNC_STATUS_ERR: c_int = 1;
pub const ASYNC_STATUS_OK: c_int = 2;
pub const ASYNC_STATUS_EAGAIN: c_int = 3;

#[allow(non_camel_case_types)]
#[cfg(ossl300)]
pub type SSL_async_callback_fn =
    Option<unsafe extern "C" fn(s: *mut SSL, arg: *mut c_void) -> c_int>;

extern "C" {
    pub fn SSL_waiting_for_async(s: *mut SSL) -> c_int;
    pub fn SSL_get_all_async_fds(s: *mut SSL, fd: *mut c_int, numfds: *mut usize) -> c_int;
    pub fn SSL_get_changed_async_fds(
        s: *mut SSL,
        addfd: *mut c_int,
        numaddfds: *mut usize,
        delfd: *mut c_int,
        numdelfds: *mut usize,
    ) -> c_int;
    #[cfg(ossl300)]
    pub fn SSL_CTX_set_async_callback(ctx: *mut SSL_CTX, callback: SSL_async_callback_fn) -> c_int;
    #[cfg(ossl300)]
    pub fn SSL_CTX_set_async_callback_arg(ctx: *mut SSL_CTX, arg: *mut c_void) -> c_int;
    #[cfg(ossl300)]
    pub fn SSL_set_async_callback(s: *mut SSL, callback: SSL_async_callback_fn) -> c_int;
    #[cfg(ossl300)]
    pub fn SSL_set_async_callback_arg(s: *mut SSL, arg: *mut c_void) -> c_int;
    #[cfg(ossl300)]
    pub fn SSL_get_async_status(s: *mut SSL) -> c_int;

}

#[allow(non_snake_case)]
pub unsafe fn SSL_get_mode(ctx: *mut SSL) -> c_long {
    SSL_ctrl(ctx, SSL_CTRL_MODE, 0, ptr::null_mut())
}
