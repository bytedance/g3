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

#[derive(Clone, Default)]
pub struct CpuAffinityImpl {}

impl CpuAffinityImpl {
    pub const fn max_cpu_id() -> usize {
        size_of::<u64>()
    }

    pub fn add_id(&mut self, _id: usize) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "cpu affinity is not supported on this system",
        ))
    }

    pub fn apply_to_local_thread(&self) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "cpu affinity is not supported on this system",
        ))
    }
}
