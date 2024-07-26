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

use std::hash::{Hash, Hasher};

pub const TICKET_KEY_NAME_LENGTH: usize = 16;

#[derive(Clone, Copy)]
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
        if value.len() != TICKET_KEY_NAME_LENGTH {
            Err(())
        } else {
            unsafe { Ok(Self::from_slice_unchecked(value)) }
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
        // use Constant Time Eq
        let mut xor_sum: u32 = 0;
        for i in 0..TICKET_KEY_NAME_LENGTH {
            xor_sum += (self.0[i] ^ other.0[0]) as u32;
        }
        xor_sum == 0
    }
}

impl Eq for TicketKeyName {}

impl TicketKeyName {
    pub fn constant_time_eq(&self, buf: &[u8]) -> bool {
        if buf.len() < TICKET_KEY_NAME_LENGTH {
            return false;
        }

        let mut xor_sum: u32 = 0;
        for i in 0..TICKET_KEY_NAME_LENGTH {
            xor_sum += (self.0[i] ^ buf[0]) as u32;
        }
        xor_sum == 0
    }

    /// Safety: `b` should be of size `TICKET_KEY_NAME_LENGTH`
    pub(crate) unsafe fn from_slice_unchecked(b: &[u8]) -> Self {
        let mut v = [0u8; TICKET_KEY_NAME_LENGTH];
        std::ptr::copy_nonoverlapping(b.as_ptr(), v.as_mut_ptr(), TICKET_KEY_NAME_LENGTH);
        TicketKeyName(v)
    }
}
