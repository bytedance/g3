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
