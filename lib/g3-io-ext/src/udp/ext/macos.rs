/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use libc::{c_int, c_uint, c_void, iovec, size_t, socklen_t, ssize_t};

#[repr(C)]
pub(super) struct msghdr_x {
    pub msg_name: *mut c_void,
    pub msg_namelen: socklen_t,
    pub msg_iov: *mut iovec,
    pub msg_iovlen: c_int,
    pub msg_control: *mut c_void,
    pub msg_controllen: socklen_t,
    pub msg_flags: c_int,
    pub msg_datalen: size_t,
}

unsafe extern "C" {
    pub(super) fn sendmsg_x(s: c_int, msgp: *mut msghdr_x, cnt: c_uint, flags: c_int) -> ssize_t;
    pub(super) fn recvmsg_x(s: c_int, msgp: *mut msghdr_x, cnt: c_uint, flags: c_int) -> ssize_t;
}
