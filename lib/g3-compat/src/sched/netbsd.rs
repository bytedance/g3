/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;

pub struct CpuAffinityImpl {
    cpu_set: *mut libc::cpuset_t,
}

unsafe impl Send for CpuAffinityImpl {}

impl Clone for CpuAffinityImpl {
    fn clone(&self) -> Self {
        let mut new = CpuAffinityImpl::default();
        for i in 0..=self.max_cpu_id() {
            let set = unsafe { libc::_cpuset_isset(i as u64, self.cpu_set) };
            if set > 0 {
                let _ = new.add_id(i);
            }
        }
        new
    }
}

impl Default for CpuAffinityImpl {
    fn default() -> Self {
        let cpu_set = unsafe { libc::_cpuset_create() };
        if cpu_set.is_null() {
            panic!("failed to create cpuset_t");
        }
        CpuAffinityImpl { cpu_set }
    }
}

impl Drop for CpuAffinityImpl {
    fn drop(&mut self) {
        if !self.cpu_set.is_null() {
            unsafe { libc::_cpuset_destroy(self.cpu_set) };
            self.cpu_set = std::ptr::null_mut();
        }
    }
}

impl CpuAffinityImpl {
    pub fn max_cpu_id(&self) -> usize {
        let bytes = unsafe { libc::_cpuset_size(self.cpu_set) };
        (bytes << 3) - 1
    }

    pub(super) fn add_id(&mut self, id: usize) -> io::Result<()> {
        unsafe {
            if libc::_cpuset_set(id as libc::cpuid_t, self.cpu_set) != 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        }
    }

    pub(super) fn apply_to_local_thread(&self) -> io::Result<()> {
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

unsafe extern "C" {
    pub fn _sched_setaffinity(
        pid: libc::pid_t,
        tid: libc::lwpid_t,
        size: libc::size_t,
        set: *const libc::cpuset_t,
    ) -> libc::c_int;
}
