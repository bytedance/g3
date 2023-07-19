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

use openssl::encrypt::Decrypter;
use openssl::hash::MessageDigest;
use openssl::nid::Nid;
use openssl::pkey::{PKey, Private};
use openssl::rsa::Padding;
use openssl::sign::{RsaPssSaltlen, Signer};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};

use super::{KeylessDataResponse, KeylessErrorResponse, KeylessPongResponse};

#[derive(Clone, Copy)]
pub(crate) enum KeylessAction {
    NotSet,
    Ping,
    RsaDecrypt(Padding),
    RsaSign(Nid),
    RsaPssSign(Nid),
    EcdsaSign(Nid),
    Ed25519Sign,
}

#[derive(Debug, Error)]
pub(crate) enum KeylessRequestError {
    #[error("read failed: {0:?}")]
    ReadFailed(io::Error),
    #[error("invalid message length")]
    InvalidMessageLength,
    #[error("unexpected version {0}.{1}")]
    UnexpectedVersion(u8, u8),
    #[error("corrupted message")]
    CorruptedMessage,
    #[error("invalid length for item {0}")]
    InvalidItemLength(u8),
    #[error("invalid op code {0}")]
    InvalidOpCode(u8),
}

pub(crate) struct KeylessRequest {
    pub(crate) id: u32,
    pub(crate) action: KeylessAction,
    pub(crate) ski: Vec<u8>,
    pub(crate) digest: Vec<u8>,
    pub(crate) payload: Vec<u8>,
}

impl KeylessRequest {
    fn new(id: u32) -> Self {
        KeylessRequest {
            id,
            action: KeylessAction::NotSet,
            ski: Vec::new(),
            digest: Vec::new(),
            payload: Vec::new(),
        }
    }

    pub(crate) async fn read<R>(
        reader: &mut R,
        buf: &mut Vec<u8>,
    ) -> Result<Self, KeylessRequestError>
    where
        R: AsyncRead + Unpin,
    {
        let mut hdr_buf = [0u8; 8];
        let len = reader
            .read_exact(&mut hdr_buf)
            .await
            .map_err(KeylessRequestError::ReadFailed)?;
        if len < 4 {
            return Err(KeylessRequestError::InvalidMessageLength);
        }

        let major = hdr_buf[0];
        let minor = hdr_buf[1];
        if major != 1 || minor != 0 {
            return Err(KeylessRequestError::UnexpectedVersion(major, minor));
        }

        let len = ((hdr_buf[2] as usize) << 8) + hdr_buf[3] as usize;
        buf.clear();
        buf.resize(len, 0);
        let nr = reader
            .read_exact(buf)
            .await
            .map_err(KeylessRequestError::ReadFailed)?;
        if nr < len {
            return Err(KeylessRequestError::InvalidMessageLength);
        }

        let id = u32::from_be_bytes([hdr_buf[4], hdr_buf[5], hdr_buf[6], hdr_buf[7]]);
        let mut request = KeylessRequest::new(id);
        request.parse_buf(buf)?;
        Ok(request)
    }

    fn parse_buf(&mut self, buf: &[u8]) -> Result<(), KeylessRequestError> {
        let total_len = buf.len();
        let mut offset = 0usize;
        loop {
            if offset + super::ITEM_HEADER_LENGTH > total_len {
                return Err(KeylessRequestError::CorruptedMessage);
            }

            let hdr = &buf[offset..offset + super::ITEM_HEADER_LENGTH];
            let item = hdr[0];
            let item_len = ((hdr[1] as usize) << 8) + hdr[2] as usize;
            if item_len < 1 {
                return Err(KeylessRequestError::InvalidItemLength(item));
            }
            offset += super::ITEM_HEADER_LENGTH;
            if offset + item_len > total_len {
                return Err(KeylessRequestError::InvalidItemLength(item));
            }

            let data = &buf[offset..offset + item_len];
            match item {
                // Cert Digest
                0x01 => {
                    self.digest = data.to_vec();
                }
                // SKI
                0x04 => {
                    self.ski = data.to_vec();
                }
                // OPCODE
                0x11 => {
                    if item_len != 1 {
                        return Err(KeylessRequestError::InvalidItemLength(item));
                    }
                    self.parse_opcode(data[0])?;
                }
                // PAYLOAD
                0x12 => {
                    self.payload = data.to_vec();
                }
                // PADDING
                0x20 => {}
                _ => {}
            }

            offset += item_len;
            if offset >= total_len {
                break;
            }
        }

        Ok(())
    }

    fn parse_opcode(&mut self, opcode: u8) -> Result<(), KeylessRequestError> {
        let action = match opcode {
            0x01 => KeylessAction::RsaDecrypt(Padding::PKCS1),
            0x02 => KeylessAction::RsaSign(Nid::MD5_SHA1),
            0x03 => KeylessAction::RsaSign(Nid::SHA1),
            0x04 => KeylessAction::RsaSign(Nid::SHA224),
            0x05 => KeylessAction::RsaSign(Nid::SHA256),
            0x06 => KeylessAction::RsaSign(Nid::SHA384),
            0x07 => KeylessAction::RsaSign(Nid::SHA512),
            0x08 => KeylessAction::RsaDecrypt(Padding::NONE),
            0x12 => KeylessAction::EcdsaSign(Nid::MD5_SHA1),
            0x13 => KeylessAction::EcdsaSign(Nid::SHA1),
            0x14 => KeylessAction::EcdsaSign(Nid::SHA224),
            0x15 => KeylessAction::EcdsaSign(Nid::SHA256),
            0x16 => KeylessAction::EcdsaSign(Nid::SHA384),
            0x17 => KeylessAction::EcdsaSign(Nid::SHA512),
            0x18 => KeylessAction::Ed25519Sign,
            0x35 => KeylessAction::RsaPssSign(Nid::SHA256),
            0x36 => KeylessAction::RsaPssSign(Nid::SHA384),
            0x37 => KeylessAction::RsaPssSign(Nid::SHA512),
            0xF1 => KeylessAction::Ping,
            n => return Err(KeylessRequestError::InvalidOpCode(n)),
        };
        self.action = action;
        Ok(())
    }

    pub(crate) fn ping_pong(&self) -> Option<KeylessPongResponse> {
        if matches!(self.action, KeylessAction::Ping) {
            Some(KeylessPongResponse::new(self.id, &self.payload))
        } else {
            None
        }
    }

    pub(crate) fn find_key(&self) -> Option<PKey<Private>> {
        if !self.ski.is_empty() {
            if let Some(k) = crate::store::get_by_ski(&self.ski) {
                return Some(k);
            }
        }
        None
    }

    pub(crate) fn process(
        &self,
        key: &PKey<Private>,
    ) -> Result<KeylessDataResponse, KeylessErrorResponse> {
        let key_size = key.size();
        let err_rsp = KeylessErrorResponse::new(self.id);
        let mut data_rsp = KeylessDataResponse::new(self.id, key_size);
        match self.action {
            KeylessAction::RsaDecrypt(p) => {
                let mut decrypter = Decrypter::new(key).map_err(|_| err_rsp.crypto_fail())?;
                decrypter
                    .set_rsa_padding(p)
                    .map_err(|_| err_rsp.crypto_fail())?;

                let len = decrypter
                    .decrypt(&self.payload, data_rsp.payload_data_mut())
                    .map_err(|_| err_rsp.crypto_fail())?;
                data_rsp.finalize_payload(len);
                Ok(data_rsp)
            }
            KeylessAction::RsaSign(h) => {
                let mut signer = Signer::new(MessageDigest::from_nid(h).unwrap(), key)
                    .map_err(|_| err_rsp.crypto_fail())?;
                signer
                    .set_rsa_padding(Padding::PKCS1)
                    .map_err(|_| err_rsp.crypto_fail())?;
                signer
                    .update(&self.payload)
                    .map_err(|_| err_rsp.crypto_fail())?;
                let len = signer
                    .sign(data_rsp.payload_data_mut())
                    .map_err(|_| err_rsp.crypto_fail())?;
                data_rsp.finalize_payload(len);
                Ok(data_rsp)
            }
            KeylessAction::RsaPssSign(h) => {
                let mut signer = Signer::new(MessageDigest::from_nid(h).unwrap(), key)
                    .map_err(|_| err_rsp.crypto_fail())?;
                signer
                    .set_rsa_padding(Padding::PKCS1_PSS)
                    .map_err(|_| err_rsp.crypto_fail())?;
                signer
                    .set_rsa_pss_saltlen(RsaPssSaltlen::DIGEST_LENGTH)
                    .map_err(|_| err_rsp.crypto_fail())?;
                signer
                    .update(&self.payload)
                    .map_err(|_| err_rsp.crypto_fail())?;
                let len = signer
                    .sign(data_rsp.payload_data_mut())
                    .map_err(|_| err_rsp.crypto_fail())?;
                data_rsp.finalize_payload(len);
                Ok(data_rsp)
            }
            KeylessAction::EcdsaSign(h) => {
                let mut signer = Signer::new(MessageDigest::from_nid(h).unwrap(), key)
                    .map_err(|_| err_rsp.crypto_fail())?;
                signer
                    .update(&self.payload)
                    .map_err(|_| err_rsp.crypto_fail())?;
                let len = signer
                    .sign(data_rsp.payload_data_mut())
                    .map_err(|_| err_rsp.crypto_fail())?;
                data_rsp.finalize_payload(len);
                Ok(data_rsp)
            }
            KeylessAction::Ed25519Sign => {
                let mut signer =
                    Signer::new_without_digest(key).map_err(|_| err_rsp.crypto_fail())?;
                signer
                    .update(&self.payload)
                    .map_err(|_| err_rsp.crypto_fail())?;
                let len = signer
                    .sign(data_rsp.payload_data_mut())
                    .map_err(|_| err_rsp.crypto_fail())?;
                data_rsp.finalize_payload(len);
                Ok(data_rsp)
            }
            KeylessAction::NotSet | KeylessAction::Ping => Err(err_rsp.unexpected_op_code()),
        }
    }
}
