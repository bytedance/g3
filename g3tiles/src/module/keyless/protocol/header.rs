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

use std::ptr;

#[derive(Clone, Copy, Default)]
pub(crate) struct KeylessHeader {
    bytes: [u8; super::KEYLESS_HEADER_LEN],
}

impl KeylessHeader {
    pub(super) fn payload_len(&self) -> u16 {
        u16::from_be_bytes([self.bytes[2], self.bytes[3]])
    }

    pub(super) fn sync_payload_len(&mut self, other: &KeylessHeader) {
        unsafe {
            ptr::copy_nonoverlapping(other.bytes[2..4].as_ptr(), self.bytes[2..4].as_mut_ptr(), 2)
        };
    }

    pub(super) fn id(&self) -> u32 {
        let mut b = [0u8; 4];
        unsafe { ptr::copy_nonoverlapping(self.bytes[4..8].as_ptr(), b.as_mut_ptr(), 4) };
        u32::from_be_bytes(b)
    }

    pub(super) fn set_id(&mut self, id: u32) {
        let b = id.to_be_bytes();
        unsafe { ptr::copy_nonoverlapping(b.as_ptr(), self.bytes[4..8].as_mut_ptr(), 4) };
    }
}

impl AsRef<[u8]> for KeylessHeader {
    fn as_ref(&self) -> &[u8] {
        self.bytes.as_ref()
    }
}

impl AsMut<[u8]> for KeylessHeader {
    fn as_mut(&mut self) -> &mut [u8] {
        self.bytes.as_mut()
    }
}
