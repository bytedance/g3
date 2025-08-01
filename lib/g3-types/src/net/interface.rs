/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::num::NonZeroU32;
use std::str::FromStr;
use std::{fmt, io, mem, ptr};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Interface {
    name: [u8; libc::IFNAMSIZ],
    id: NonZeroU32,
    len: usize,
}

impl Interface {
    pub fn name(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(&self.name[..self.len - 1]) }
    }

    #[inline]
    pub fn id(&self) -> NonZeroU32 {
        self.id
    }

    /// Get the '\0' pended raw value
    pub fn c_bytes(&self) -> &[u8] {
        &self.name[..self.len]
    }
}

impl TryFrom<u32> for Interface {
    type Error = io::Error;

    fn try_from(id: u32) -> io::Result<Self> {
        match NonZeroU32::try_from(id) {
            Ok(id) => Self::try_from(id),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "interface id should not be zero",
            )),
        }
    }
}

impl TryFrom<NonZeroU32> for Interface {
    type Error = io::Error;

    fn try_from(id: NonZeroU32) -> io::Result<Self> {
        let mut buffer: [libc::c_char; libc::IFNAMSIZ] = unsafe { mem::zeroed() };

        let ptr = unsafe { libc::if_indextoname(id.get(), buffer.as_mut_ptr()) };
        if ptr.is_null() {
            return Err(io::Error::last_os_error());
        }
        let c_name = unsafe { std::ffi::CStr::from_ptr(ptr) };
        if let Err(e) = c_name.to_str() {
            return Err(io::Error::other(format!(
                "interface {id} name is not valid utf-8: {e}"
            )));
        }
        let len = c_name.to_bytes_with_nul().len();

        let mut name = [0u8; libc::IFNAMSIZ];
        unsafe {
            ptr::copy_nonoverlapping(buffer.as_ptr(), name.as_mut_ptr() as _, len);
        }

        Ok(Interface { name, id, len })
    }
}

impl FromStr for Interface {
    type Err = io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(id) = NonZeroU32::from_str(s) {
            Self::try_from(id)
        } else {
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

            let id = unsafe { libc::if_nametoindex(name.as_ptr().cast()) };
            match NonZeroU32::new(id as u32) {
                Some(id) => Ok(Interface {
                    name,
                    id,
                    len: s.len() + 1,
                }),
                None => Err(io::Error::last_os_error()),
            }
        }
    }
}

impl fmt::Display for Interface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(target_os = "linux", target_os = "android"))]
    const LOOPBACK_INTERFACE: &str = "lo";
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    const LOOPBACK_INTERFACE: &str = "lo0";

    #[test]
    fn lo_by_name() {
        let iface = Interface::from_str(LOOPBACK_INTERFACE).unwrap();
        let iface_id = iface.id.get();
        let bytes = iface.c_bytes();
        let len = bytes.len();
        assert_eq!(len, LOOPBACK_INTERFACE.len() + 1);
        assert_eq!(&bytes[..len - 1], LOOPBACK_INTERFACE.as_bytes());
        assert_eq!(bytes[len - 1], 0);

        let iface = Interface::try_from(iface_id).unwrap();
        assert_eq!(iface.name(), LOOPBACK_INTERFACE);
    }

    #[test]
    fn lo_by_id() {
        let iface = Interface::try_from(1).unwrap();
        let iface_name = iface.name();

        let iface = Interface::from_str(iface_name).unwrap();
        assert_eq!(iface.id.get(), 1);
    }
}
