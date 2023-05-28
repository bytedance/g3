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

pub struct CpuAffinity {
    cpu_set: *mut libc::cpuset_t,
}

unsafe impl Send for CpuAffinity {}

impl Clone for CpuAffinity {
    fn clone(&self) -> Self {
        let mut new = CpuAffinity::default();
        for i in 0..=self.max_cpu_id() {
            let set = unsafe { libc::_cpuset_isset(i, self.cpu_set) };
            if set > 0 {
                let _ = new.add_id(i);
            }
        }
        new
    }
}

impl Default for CpuAffinity {
    fn default() -> Self {
        let cpu_set = unsafe { libc::_cpuset_create() };
        if cpu_set.is_null() {
            panic!("failed to create cpuset_t");
        }
        CpuAffinity { cpu_set }
    }
}

impl Drop for CpuAffinity {
    fn drop(&mut self) {
        if !self.cpu_set.is_null() {
            unsafe { libc::_cpuset_destroy(self.cpu_set) };
            self.cpu_set = std::ptr::null_mut();
        }
    }
}

impl CpuAffinity {
    fn max_cpu_id(&self) -> usize {
        let bytes = unsafe { libc::_cpuset_size(self.cpu_set) };
        (bytes << 3) - 1
    }

    pub fn add_id(&mut self, id: usize) -> io::Result<()> {
        unsafe {
            if libc::_cpuset_set(id as libc::cpuid_t, self.cpu_set) != 0 {
                return Err(io::Error::last_os_error());
            } else {
                Ok(())
            }
        }
    }

    pub fn apply_to_local_thread(&self) -> io::Result<()> {
        let r = unsafe {
            _sched_setaffinity(
                -1,
                libc::_lwp_self(),
                libc::_cpuset_size(self.cpu_set),
                self.cpu_set,
            )
        };
        if r != 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

extern "C" {
    pub fn _sched_setaffinity(
        pid: libc::pid_t,
        tid: libc::lwpid_t,
        size: libc::size_t,
        set: *const libc::cpuset_t,
    ) -> libc::c_int;
}
