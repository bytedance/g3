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
use std::num::NonZeroI32;

use libc::{
    mach_thread_self, thread_affinity_policy, thread_policy_flavor_t, thread_policy_set,
    thread_policy_t, THREAD_AFFINITY_POLICY, THREAD_AFFINITY_POLICY_COUNT,
    THREAD_AFFINITY_TAG_NULL,
};

#[derive(Clone)]
pub struct CpuAffinity {
    cpu_tag: libc::integer_t,
}

impl Default for CpuAffinity {
    fn default() -> Self {
        CpuAffinity {
            cpu_tag: THREAD_AFFINITY_TAG_NULL,
        }
    }
}

impl CpuAffinity {
    pub fn new(tag: NonZeroI32) -> Self {
        CpuAffinity { cpu_tag: tag.get() }
    }

    pub fn apply_to_local_thread(&self) -> io::Result<()> {
        let mut policy_info = thread_affinity_policy {
            affinity_tag: self.cpu_tag,
        };
        let errno = unsafe {
            thread_policy_set(
                mach_thread_self(),
                THREAD_AFFINITY_POLICY as thread_policy_flavor_t,
                &mut policy_info as *mut thread_affinity_policy as thread_policy_t,
                THREAD_AFFINITY_POLICY_COUNT,
            )
        };
        match errno {
            0 => Ok(()),
            46 => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "thread_policy_set() is not supported",
            )),
            n => Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "thread_policy_set({}) returned error code {n}",
                    self.cpu_tag
                ),
            )),
        }
    }
}
