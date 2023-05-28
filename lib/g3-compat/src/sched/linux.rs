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

use std::io;
use std::mem;

#[derive(Clone)]
pub struct CpuAffinity {
    cpu_set: libc::cpu_set_t,
}

impl Default for CpuAffinity {
    fn default() -> Self {
        CpuAffinity {
            cpu_set: unsafe { mem::zeroed() },
        }
    }
}

impl CpuAffinity {
    fn max_cpu_id() -> usize {
        let bytes = mem::size_of::<libc::cpu_set_t>();
        bytes << 3
    }

    pub fn add_id(&mut self, id: usize) -> io::Result<()> {
        if id > CpuAffinity::max_cpu_id() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid cpu id",
            ));
        }
        unsafe {
            libc::CPU_SET(id, &mut self.cpu_set);
        }
        Ok(())
    }

    pub fn apply_to_local_thread(&self) -> io::Result<()> {
        let r = unsafe {
            libc::sched_setaffinity(
                0,
                mem::size_of::<libc::cpu_set_t>() as libc::size_t,
                &self.cpu_set,
            )
        };
        if r != 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}
