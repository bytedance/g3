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
use std::str::FromStr;

#[cfg_attr(any(target_os = "linux", target_os = "android"), path = "linux.rs")]
#[cfg_attr(
    any(target_os = "freebsd", target_os = "dragonfly"),
    path = "freebsd.rs"
)]
#[cfg_attr(target_os = "netbsd", path = "netbsd.rs")]
#[cfg_attr(windows, path = "windows.rs")]
mod os;
use os::CpuAffinityImpl;

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct CpuId(usize);

impl FromStr for CpuId {
    type Err = io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = usize::from_str(s).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid CPU ID {s}: {e}"),
            )
        })?;
        Ok(CpuId(id))
    }
}

#[derive(Clone)]
pub struct CpuAffinity {
    os_impl: CpuAffinityImpl,
    cpu_id_list: Vec<usize>,
    max_cpu_id: usize,
}

impl Default for CpuAffinity {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuAffinity {
    pub fn new() -> Self {
        let os_impl = CpuAffinityImpl::default();
        let max_cpu_id = os_impl.max_cpu_id();
        CpuAffinity {
            os_impl,
            cpu_id_list: Vec::new(),
            max_cpu_id,
        }
    }

    pub fn cpu_id_list(&self) -> &[usize] {
        &self.cpu_id_list
    }

    pub fn add_id(&mut self, id: usize) -> io::Result<()> {
        if id > self.max_cpu_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid CPU ID, the max allowed is {}", self.max_cpu_id),
            ));
        }
        self.os_impl.add_id(id)?;
        self.cpu_id_list.push(id);
        Ok(())
    }

    pub fn parse_add(&mut self, s: &str) -> io::Result<()> {
        for p in s.split(',') {
            let part = p.trim();
            if part.is_empty() {
                continue;
            }

            match part.split_once('-') {
                Some((s1, s2)) => {
                    let start = CpuId::from_str(s1)?;
                    let end = CpuId::from_str(s2)?;
                    if start >= end {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("invalid CPU ID range {part}"),
                        ));
                    }
                    for id in start.0..=end.0 {
                        self.add_id(id)?;
                    }
                }
                None => {
                    let id = CpuId::from_str(part)?;
                    self.add_id(id.0)?;
                }
            }
        }
        Ok(())
    }

    pub fn apply_to_local_thread(&self) -> io::Result<()> {
        self.os_impl.apply_to_local_thread()
    }
}

#[cfg(all(
    test,
    any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        windows,
    )
))]
mod tests {
    use super::*;

    #[test]
    fn single() {
        let mut affinity = CpuAffinity::default();
        assert!(affinity.cpu_id_list().is_empty());
        affinity.add_id(1).unwrap();
        assert_eq!(affinity.cpu_id_list(), &[1]);
    }

    #[test]
    fn many() {
        let mut affinity = CpuAffinity::default();
        affinity.add_id(2).unwrap();
        affinity.parse_add("0").unwrap();
        assert_eq!(affinity.cpu_id_list(), &[2, 0]);
    }

    #[test]
    fn range() {
        let mut affinity = CpuAffinity::default();
        affinity.parse_add("0-1,4").unwrap();
        assert_eq!(affinity.cpu_id_list(), &[0, 1, 4]);
    }
}
