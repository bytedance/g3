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

use super::PacketParseError;

const MAX_PACKET_NUMBER_BYTES: usize = 4;

pub struct Header {
    pub byte1: u8,
    packet_number_buf: [u8; MAX_PACKET_NUMBER_BYTES],
    pub packet_number_len: usize,
    pub packet_number: u32,
}

impl Header {
    pub(super) fn decode_long(
        byte1: u8,
        mask: [u8; 5],
        data: &[u8],
    ) -> Result<Self, PacketParseError> {
        let byte1_low_bits = (byte1 ^ mask[0]) & 0x0F;
        let real_byte1 = byte1_low_bits | (byte1 & 0xF0);
        let packet_number_len = (byte1_low_bits & 0b0000_0011) + 1;
        if packet_number_len as usize > MAX_PACKET_NUMBER_BYTES {
            return Err(PacketParseError::InvalidPacketNumberLength(
                packet_number_len,
            ));
        }
        let mut packet_number_bytes = [0u8; MAX_PACKET_NUMBER_BYTES];
        for i in 0..packet_number_len as usize {
            packet_number_bytes[MAX_PACKET_NUMBER_BYTES - packet_number_len as usize + i] =
                mask[i + 1] ^ data[i];
        }
        let packet_number = u32::from_be_bytes(packet_number_bytes);
        Ok(Header {
            byte1: real_byte1,
            packet_number_buf: packet_number_bytes,
            packet_number_len: packet_number_len as usize,
            packet_number,
        })
    }

    pub(super) fn packet_number_bytes(&self) -> &[u8] {
        &self.packet_number_buf[MAX_PACKET_NUMBER_BYTES - self.packet_number_len..]
    }

    pub(super) fn xor_nonce(&self, iv: &[u8; 12]) -> [u8; 12] {
        let mut nonce = *iv;
        nonce[8] ^= self.packet_number_buf[0];
        nonce[9] ^= self.packet_number_buf[1];
        nonce[10] ^= self.packet_number_buf[2];
        nonce[11] ^= self.packet_number_buf[3];
        nonce
    }
}
