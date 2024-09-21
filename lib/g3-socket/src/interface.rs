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

use std::str::FromStr;
use std::{fmt, io, ptr};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InterfaceName {
    name: [u8; libc::IFNAMSIZ],
    len: usize,
}

impl InterfaceName {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.name[..self.len]
    }

    pub fn as_str(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(&self.name[..self.len - 1]) }
    }
}

impl FromStr for InterfaceName {
    type Err = io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() + 1 > libc::IFNAMSIZ {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "too long interface name",
            ));
        };

        let mut name = [0u8; libc::IFNAMSIZ];
        unsafe {
            ptr::copy_nonoverlapping(s.as_ptr(), name.as_mut_ptr(), s.len());
        }
        name[s.len()] = 0;

        Ok(InterfaceName {
            name,
            len: s.len() + 1,
        })
    }
}

impl fmt::Display for InterfaceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
