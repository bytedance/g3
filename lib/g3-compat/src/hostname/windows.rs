/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;

use windows_sys::Win32::System::SystemInformation;

pub fn hostname() -> OsString {
    let mut size = 0;
    unsafe {
        SystemInformation::GetComputerNameExW(
            SystemInformation::ComputerNamePhysicalDnsHostname,
            std::ptr::null_mut(),
            &mut size,
        );
    }

    let mut buffer = vec![0u16; size as usize];
    unsafe {
        SystemInformation::GetComputerNameExW(
            SystemInformation::ComputerNamePhysicalDnsHostname,
            buffer.as_mut_ptr(),
            &mut size,
        );
    }

    OsString::from_wide(&buffer)
}
