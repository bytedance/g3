/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;

pub fn hostname() -> OsString {
    let uname = rustix::system::uname();
    OsStr::from_bytes(uname.nodename().to_bytes()).to_os_string()
}
