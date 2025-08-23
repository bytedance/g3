/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
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
