/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::hash::{Hash, Hasher};

pub const TICKET_KEY_NAME_LENGTH: usize = 16;

#[derive(Clone, Copy, Debug)]
pub struct TicketKeyName([u8; TICKET_KEY_NAME_LENGTH]);

impl From<[u8; TICKET_KEY_NAME_LENGTH]> for TicketKeyName {
    fn from(value: [u8; TICKET_KEY_NAME_LENGTH]) -> Self {
        TicketKeyName(value)
    }
}

impl AsRef<[u8]> for TicketKeyName {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl TryFrom<&[u8]> for TicketKeyName {
    type Error = ();

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() < TICKET_KEY_NAME_LENGTH {
            Err(())
        } else {
            unsafe { Ok(Self::from_slice_unchecked(&value[..TICKET_KEY_NAME_LENGTH])) }
        }
    }
}

impl Hash for TicketKeyName {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl PartialEq for TicketKeyName {
    fn eq(&self, other: &Self) -> bool {
        constant_time_eq::constant_time_eq_n(&self.0, &other.0)
    }
}

impl Eq for TicketKeyName {}

impl TicketKeyName {
    pub fn constant_time_eq(&self, buf: &[u8]) -> bool {
        if buf.len() < TICKET_KEY_NAME_LENGTH {
            return false;
        }

        constant_time_eq::constant_time_eq(&self.0, buf)
    }

    /// Safety: `b` should be of size `TICKET_KEY_NAME_LENGTH`
    pub(crate) unsafe fn from_slice_unchecked(b: &[u8]) -> Self {
        unsafe {
            let mut v = [0u8; TICKET_KEY_NAME_LENGTH];
            std::ptr::copy_nonoverlapping(b.as_ptr(), v.as_mut_ptr(), TICKET_KEY_NAME_LENGTH);
            TicketKeyName(v)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_eq() {
        let name1 = TicketKeyName([0u8; 16]);
        let name2 = TicketKeyName([0u8; 16]);
        assert_eq!(name1, name2);
        assert!(name1.constant_time_eq(name2.as_ref()));
    }
}
