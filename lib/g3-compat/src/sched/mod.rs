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

#[cfg_attr(any(target_os = "linux", target_os = "android"), path = "linux.rs")]
#[cfg_attr(
    any(target_os = "freebsd", target_os = "dragonfly"),
    path = "freebsd.rs"
)]
#[cfg_attr(target_os = "netbsd", path = "netbsd.rs")]
mod os;
use os::CpuAffinityImpl;

const MAX_CPU_ID: usize = CpuAffinityImpl::max_cpu_id();

#[derive(Clone, Default)]
pub struct CpuAffinity {
    os_impl: CpuAffinityImpl,
    cpu_id_list: Vec<usize>,
}

impl CpuAffinity {
    pub fn cpu_id_list(&self) -> &[usize] {
        &self.cpu_id_list
    }

    pub fn add_id(&mut self, id: usize) -> io::Result<()> {
        if id > MAX_CPU_ID {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid cpu id, the max allowed is {MAX_CPU_ID}"),
            ));
        }
        self.os_impl.add_id(id)?;
        self.cpu_id_list.push(id);
        Ok(())
    }

    pub fn apply_to_local_thread(&self) -> io::Result<()> {
        self.os_impl.apply_to_local_thread()
    }
}
