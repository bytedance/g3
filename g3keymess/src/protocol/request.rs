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
use openssl::md::Md;
use openssl::nid::Nid;
use openssl::pkey::{PKey, Private};
use openssl::pkey_ctx::PkeyCtx;
use openssl::rsa::Padding;
use openssl::sign::RsaPssSaltlen;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};

use g3_types::net::{T1L2BVParse, TlvParse};

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
    #[error("closed early")]
    ClosedEarly,
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
}

pub(crate) struct KeylessRequest {
    pub(crate) id: u32,
    pub(crate) opcode: u8,
    pub(crate) action: KeylessAction,
    pub(crate) ski: Vec<u8>,
    pub(crate) payload: Vec<u8>,
}

impl T1L2BVParse<'_> for KeylessRequest {
    type Error = KeylessRequestError;

    fn no_enough_data() -> Self::Error {
        KeylessRequestError::CorruptedMessage
    }

    fn parse_value(&mut self, tag: u8, v: &[u8]) -> Result<(), Self::Error> {
        match tag {
            // Cert Digest
            0x01 => {}
            // SKI
            0x04 => {
                self.ski = v.to_vec();
            }
            // OPCODE
            0x11 => {
                if v.len() != 1 {
                    return Err(KeylessRequestError::InvalidItemLength(tag));
                }
                self.opcode = v[0];
            }
            // PAYLOAD
            0x12 => {
                self.payload = v.to_vec();
            }
            // PADDING
            0x20 => {}
            _ => {}
        }
        Ok(())
    }
}

impl KeylessRequest {
    fn new(id: u32) -> Self {
        KeylessRequest {
            id,
            opcode: 0,
            action: KeylessAction::NotSet,
            ski: Vec::new(),
            payload: Vec::new(),
        }
    }

    pub(crate) async fn read<R>(
        reader: &mut R,
        buf: &mut Vec<u8>,
        msg_count: usize,
    ) -> Result<Self, KeylessRequestError>
    where
        R: AsyncRead + Unpin,
    {
        const HDR_BUF_LEN: usize = 8;

        let mut hdr_buf = [0u8; HDR_BUF_LEN];
        match reader.read_exact(&mut hdr_buf).await {
            Ok(len) => {
                if len < HDR_BUF_LEN {
                    return if msg_count == 0 {
                        Err(KeylessRequestError::ClosedEarly)
                    } else {
                        Err(KeylessRequestError::InvalidMessageLength)
                    };
                }
            }
            Err(e) => {
                return if msg_count == 0 {
                    Err(KeylessRequestError::ClosedEarly)
                } else {
                    Err(KeylessRequestError::ReadFailed(e))
                };
            }
        }

        let major = hdr_buf[0];
        let minor = hdr_buf[1];
        if major != 1 || minor != 0 {
            return Err(KeylessRequestError::UnexpectedVersion(major, minor));
        }

        let len = ((hdr_buf[2] as usize) << 8) + hdr_buf[3] as usize;
        buf.clear();
        buf.resize(len, 0);
        match reader.read_exact(buf).await {
            Ok(nr) => {
                if nr < len {
                    return if msg_count == 0 {
                        Err(KeylessRequestError::ClosedEarly)
                    } else {
                        Err(KeylessRequestError::InvalidMessageLength)
                    };
                }
            }
            Err(e) => {
                return if msg_count == 0 {
                    Err(KeylessRequestError::ClosedEarly)
                } else {
                    Err(KeylessRequestError::ReadFailed(e))
                };
            }
        }

        let id = u32::from_be_bytes([hdr_buf[4], hdr_buf[5], hdr_buf[6], hdr_buf[7]]);
        let mut request = KeylessRequest::new(id);
        request.parse_tlv(buf)?;
        Ok(request)
    }

    pub(crate) fn verify_opcode(&mut self) -> Result<(), KeylessErrorResponse> {
        let action = match self.opcode {
            0x01 => KeylessAction::RsaDecrypt(Padding::PKCS1),
            0x02 => {
                self.check_payload_for_message_digest(
                    MessageDigest::from_nid(Nid::MD5_SHA1).unwrap(),
                )?;
                KeylessAction::RsaSign(Nid::MD5_SHA1)
            }
            0x03 => {
                self.check_payload_for_message_digest(MessageDigest::sha1())?;
                KeylessAction::RsaSign(Nid::SHA1)
            }
            0x04 => {
                self.check_payload_for_message_digest(MessageDigest::sha224())?;
                KeylessAction::RsaSign(Nid::SHA224)
            }
            0x05 => {
                self.check_payload_for_message_digest(MessageDigest::sha256())?;
                KeylessAction::RsaSign(Nid::SHA256)
            }
            0x06 => {
                self.check_payload_for_message_digest(MessageDigest::sha384())?;
                KeylessAction::RsaSign(Nid::SHA384)
            }
            0x07 => {
                self.check_payload_for_message_digest(MessageDigest::sha512())?;
                KeylessAction::RsaSign(Nid::SHA512)
            }
            0x08 => KeylessAction::RsaDecrypt(Padding::NONE),
            0x12 => {
                self.check_payload_for_message_digest(
                    MessageDigest::from_nid(Nid::MD5_SHA1).unwrap(),
                )?;
                KeylessAction::EcdsaSign(Nid::MD5_SHA1)
            }
            0x13 => {
                self.check_payload_for_message_digest(MessageDigest::sha1())?;
                KeylessAction::EcdsaSign(Nid::SHA1)
            }
            0x14 => {
                self.check_payload_for_message_digest(MessageDigest::sha224())?;
                KeylessAction::EcdsaSign(Nid::SHA224)
            }
            0x15 => {
                self.check_payload_for_message_digest(MessageDigest::sha256())?;
                KeylessAction::EcdsaSign(Nid::SHA256)
            }
            0x16 => {
                self.check_payload_for_message_digest(MessageDigest::sha384())?;
                KeylessAction::EcdsaSign(Nid::SHA384)
            }
            0x17 => {
                self.check_payload_for_message_digest(MessageDigest::sha512())?;
                KeylessAction::EcdsaSign(Nid::SHA512)
            }
            0x18 => KeylessAction::Ed25519Sign,
            0x35 => {
                self.check_payload_for_message_digest(MessageDigest::sha256())?;
                KeylessAction::RsaPssSign(Nid::SHA256)
            }
            0x36 => {
                self.check_payload_for_message_digest(MessageDigest::sha384())?;
                KeylessAction::RsaPssSign(Nid::SHA384)
            }
            0x37 => {
                self.check_payload_for_message_digest(MessageDigest::sha512())?;
                KeylessAction::RsaPssSign(Nid::SHA512)
            }
            0xF1 => KeylessAction::Ping,
            _ => return Err(KeylessErrorResponse::new(self.id).bad_op_code()),
        };
        self.action = action;
        Ok(())
    }

    fn check_payload_for_message_digest(
        &self,
        d: MessageDigest,
    ) -> Result<(), KeylessErrorResponse> {
        if d.size() != self.payload.len() {
            return Err(KeylessErrorResponse::new(self.id).format_error());
        }
        Ok(())
    }

    fn check_payload_for_key_size(&self, key_size: usize) -> Result<(), KeylessErrorResponse> {
        match self.opcode {
            0x01 | 0x08 => {
                if self.payload.len() != key_size {
                    return Err(KeylessErrorResponse::new(self.id).format_error());
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn ping_pong(&self) -> Option<KeylessPongResponse> {
        if matches!(self.action, KeylessAction::Ping) {
            Some(KeylessPongResponse::new(self.id, &self.payload))
        } else {
            None
        }
    }

    pub(crate) fn find_key(&self) -> Result<PKey<Private>, KeylessErrorResponse> {
        if !self.ski.is_empty() {
            if let Some(k) = crate::store::get_by_ski(&self.ski) {
                self.check_payload_for_key_size(k.size())?;
                return Ok(k);
            }
        }
        Err(KeylessErrorResponse::new(self.id).key_not_found())
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
                let mut ctx = PkeyCtx::new(key).map_err(|_| err_rsp.crypto_fail())?;
                ctx.sign_init().map_err(|_| err_rsp.crypto_fail())?;
                ctx.set_signature_md(Md::from_nid(h).unwrap())
                    .map_err(|_| err_rsp.crypto_fail())?;
                ctx.set_rsa_padding(Padding::PKCS1)
                    .map_err(|_| err_rsp.crypto_fail())?;

                let len = ctx
                    .sign(&self.payload, Some(data_rsp.payload_data_mut()))
                    .map_err(|_| err_rsp.crypto_fail())?;
                data_rsp.finalize_payload(len);
                Ok(data_rsp)
            }
            KeylessAction::RsaPssSign(h) => {
                let mut ctx = PkeyCtx::new(key).map_err(|_| err_rsp.crypto_fail())?;
                ctx.sign_init().map_err(|_| err_rsp.crypto_fail())?;
                ctx.set_signature_md(Md::from_nid(h).unwrap())
                    .map_err(|_| err_rsp.crypto_fail())?;
                ctx.set_rsa_padding(Padding::PKCS1_PSS)
                    .map_err(|_| err_rsp.crypto_fail())?;
                ctx.set_rsa_pss_saltlen(RsaPssSaltlen::DIGEST_LENGTH)
                    .map_err(|_| err_rsp.crypto_fail())?;

                let len = ctx
                    .sign(&self.payload, Some(data_rsp.payload_data_mut()))
                    .map_err(|_| err_rsp.crypto_fail())?;
                data_rsp.finalize_payload(len);
                Ok(data_rsp)
            }
            KeylessAction::EcdsaSign(h) => {
                let mut ctx = PkeyCtx::new(key).map_err(|_| err_rsp.crypto_fail())?;
                ctx.sign_init().map_err(|_| err_rsp.crypto_fail())?;
                ctx.set_signature_md(Md::from_nid(h).unwrap())
                    .map_err(|_| err_rsp.crypto_fail())?;

                let len = ctx
                    .sign(&self.payload, Some(data_rsp.payload_data_mut()))
                    .map_err(|_| err_rsp.crypto_fail())?;
                data_rsp.finalize_payload(len);
                Ok(data_rsp)
            }
            KeylessAction::Ed25519Sign => {
                let mut ctx = PkeyCtx::new(key).map_err(|_| err_rsp.crypto_fail())?;
                ctx.sign_init().map_err(|_| err_rsp.crypto_fail())?;

                let len = ctx
                    .sign(&self.payload, Some(data_rsp.payload_data_mut()))
                    .map_err(|_| err_rsp.crypto_fail())?;
                data_rsp.finalize_payload(len);
                Ok(data_rsp)
            }
            KeylessAction::NotSet | KeylessAction::Ping => Err(err_rsp.unexpected_op_code()),
        }
    }
}
