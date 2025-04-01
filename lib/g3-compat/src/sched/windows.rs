/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use std::io;

use windows_sys::Win32::System::Threading;

#[derive(Clone, Default)]
pub struct CpuAffinityImpl {
    cpu_id_list: Vec<u32>,
}

pub const fn max_cpu_id() -> usize {
    u32::MAX as usize
}

impl CpuAffinityImpl {
    pub const fn max_cpu_id(&self) -> usize {
        max_cpu_id()
    }

    pub(super) fn add_id(&mut self, id: usize) -> io::Result<()> {
        let id = u32::try_from(id)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "out of range CPU ID {id}"))?;
        self.cpu_id_list.push(id);
        Ok(())
    }

    pub(super) fn apply_to_local_thread(&self) -> io::Result<()> {
        let len = self.cpu_id_list.len() as u32;
        let r = unsafe {
            Threading::SetThreadSelectedCpuSets(
                Threading::GetCurrentThread(),
                self.cpu_id_list.as_ptr(),
                len,
            )
        };
        if r == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}
