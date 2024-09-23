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

use openssl::error::ErrorStack;

use super::{Header, PacketParseError};
use crate::parser::quic::VarInt;

const INITIAL_SALT: &[u8] = &[
    0x38, 0x76, 0x2c, 0xf7, 0xf5, 0x59, 0x34, 0xb3, 0x4d, 0x17, 0x9a, 0xe6, 0xa4, 0xc8, 0x0c, 0xad,
    0xcc, 0xbb, 0x7f, 0x0a,
];

pub struct InitialPacketV1 {
    pub(super) packet_number: u32,
    pub(super) payload: Vec<u8>,
}

impl InitialPacketV1 {
    /// Parse a QUIC v1 Initial Packet
    ///
    /// According to https://datatracker.ietf.org/doc/html/rfc9000#name-packets-and-frames
    pub(super) fn parse_client(data: &[u8]) -> Result<Self, PacketParseError> {
        let byte1 = data[0];
        if byte1 & 0b0011_0000 != 0b0000_0000 {
            return Err(PacketParseError::InvalidLongPacketType);
        }
        let mut offset = super::LONG_PACKET_FIXED_LEN;

        // Destination Connection ID
        let left = &data[offset..];
        if left.is_empty() {
            return Err(PacketParseError::TooSmall);
        }
        let dst_cid_len = left[0] as usize;
        if dst_cid_len > 20 {
            return Err(PacketParseError::InvalidConnectionIdLength(data[0]));
        }
        let start = offset + 1;
        let end = start + dst_cid_len;
        if data.len() < end {
            return Err(PacketParseError::TooSmall);
        }
        let dst_cid = &data[start..end];
        offset = end;

        // Source Connection ID
        let left = &data[offset..];
        if left.is_empty() {
            return Err(PacketParseError::TooSmall);
        }
        let src_cid_len = left[0] as usize;
        if src_cid_len > 0 {
            offset += 1 + src_cid_len;
            if data.len() < offset {
                return Err(PacketParseError::TooSmall);
            }
        } else {
            offset += 1;
        }

        // Token
        let left = &data[offset..];
        let Some(token_len) = VarInt::try_parse(left) else {
            return Err(PacketParseError::TooSmall);
        };
        let start = offset + token_len.encoded_len();
        if start as u64 + token_len.value() > data.len() as u64 {
            return Err(PacketParseError::InvalidTokenLength(token_len.value()));
        }
        offset = start + token_len.value() as usize;

        // Length
        let left = &data[offset..];
        let Some(length) = VarInt::try_parse(left) else {
            return Err(PacketParseError::TooSmall);
        };
        offset += length.encoded_len();
        if offset as u64 + length.value() != data.len() as u64 {
            return Err(PacketParseError::InvalidLengthValue(length.value()));
        }

        let left = &data[offset..];
        if left.len() < 20 {
            // 4 offset (maybe packet number) and 16 bytes sample
            return Err(PacketParseError::InvalidLengthValue(length.value()));
        }
        let pn_offset = offset;
        let sample = &left[4..20];

        let secrets = ClientSecrets::new(dst_cid)?;
        let mask = super::aes::aes_ecb_mask(&secrets.hp, sample)?;
        let header = Header::decode_long(byte1, mask, left)?;

        let nonce = header.xor_nonce(&secrets.iv);
        let aad_vec = [
            &[header.byte1],
            &data[1..pn_offset],
            header.packet_number_bytes(),
        ];
        let tag_start = left.len() - 16;
        let ciphertext = &left[header.packet_number_len..tag_start];
        let tag = &left[tag_start..];

        let payload = super::aes::aes_gcm_decrypt(&secrets.key, &nonce, &aad_vec, ciphertext, tag)?;

        Ok(InitialPacketV1 {
            packet_number: header.packet_number,
            payload,
        })
    }
}

struct ClientSecrets {
    key: [u8; 16],
    iv: [u8; 12],
    hp: [u8; 16],
}

impl ClientSecrets {
    fn new(cid: &[u8]) -> Result<Self, ErrorStack> {
        let mut client_initial_secret = [0u8; 32];
        super::quic_hkdf_extract_expand(
            INITIAL_SALT,
            cid,
            b"client in",
            &mut client_initial_secret,
        )?;

        let mut key = [0u8; 16];
        super::quic_hkdf_expand(&client_initial_secret, b"quic key", &mut key)?;

        let mut iv = [0u8; 12];
        super::quic_hkdf_expand(&client_initial_secret, b"quic iv", &mut iv)?;

        let mut hp = [0u8; 16];
        super::quic_hkdf_expand(&client_initial_secret, b"quic hp", &mut hp)?;

        Ok(ClientSecrets { key, iv, hp })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;

    #[test]
    fn gen_secret() {
        // https://datatracker.ietf.org/doc/html/rfc9001#section-a.2
        let cid = hex!("8394c8f03e515708");
        let key = hex!("1f369613dd76d5467730efcbe3b1a22d");
        let iv = hex!("fa044b2f42a3fd3b46fb255c");
        let hp = hex!("9f50449e04a0e810283a1e9933adedd2");

        let secrets = ClientSecrets::new(&cid).unwrap();
        assert_eq!(secrets.key, key);
        assert_eq!(secrets.iv, iv);
        assert_eq!(secrets.hp, hp);
    }
}
