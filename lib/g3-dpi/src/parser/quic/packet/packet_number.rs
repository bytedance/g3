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

pub struct PacketNumber {
    pub byte1: u8,
    pub bytes: [u8; 4],
    pub raw_len: usize,
    pub value: u32,
}

impl PacketNumber {
    pub fn decode_long(byte1: u8, mask: [u8; 5], data: &[u8]) -> Result<Self, PacketParseError> {
        let byte1_low_bits = (byte1 ^ mask[0]) & 0x0F;
        let real_byte1 = byte1_low_bits | (byte1 & 0xF0);
        let packet_number_len = (byte1_low_bits & 0b0000_0011) + 1;
        if packet_number_len > 4 {
            return Err(PacketParseError::InvalidPacketNumberLength(
                packet_number_len,
            ));
        }
        let mut packet_number_bytes = [0u8; 4];
        for i in 0..packet_number_len as usize {
            packet_number_bytes[4 - packet_number_len as usize + i] = mask[i + 1] ^ data[i];
        }
        let packet_number = u32::from_be_bytes(packet_number_bytes);
        Ok(PacketNumber {
            byte1: real_byte1,
            bytes: packet_number_bytes,
            raw_len: packet_number_len as usize,
            value: packet_number,
        })
    }

    pub fn recover_header(&self, data: &[u8], pn_offset: usize) -> Vec<u8> {
        let data_header_len = pn_offset + self.raw_len;
        let mut header = Vec::with_capacity(data_header_len);
        header.push(self.byte1);
        header.extend_from_slice(&data[1..pn_offset]);
        header.extend_from_slice(&self.bytes[4 - self.raw_len..]);
        header
    }

    pub fn recover_nonce(&self, iv: &[u8; 12]) -> [u8; 12] {
        let mut nonce = *iv;
        nonce[8] ^= self.bytes[0];
        nonce[9] ^= self.bytes[1];
        nonce[10] ^= self.bytes[2];
        nonce[11] ^= self.bytes[3];
        nonce
    }
}
