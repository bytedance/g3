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

use anyhow::anyhow;
use bytes::BufMut;
use openssl::hash::DigestBytes;

use crate::target::keyless::opts::{KeylessAction, KeylessRsaPadding, KeylessSignDigest};

#[non_exhaustive]
#[repr(u8)]
#[derive(Clone, Copy)]
pub(crate) enum KeylessOpCode {
    // requests an RSA decrypted payload
    RsaDecrypt = 0x01,
    // requests an RSA signature on an MD5SHA1 hash payload
    RsaSignMd5Sha1 = 0x02,
    // requests an RSA signature on an SHA1 hash payload
    RsaSignSha1 = 0x03,
    // requests an RSA signature on an SHA224 hash payload
    RsaSignSha224 = 0x04,
    // requests an RSA signature on an SHA256 hash payload
    RsaSignSha256 = 0x05,
    // requests an RSA signature on an SHA384 hash payload
    RsaSignSha384 = 0x06,
    // requests an RSA signature on an SHA512 hash payload
    RsaSignSha512 = 0x07,
    // requests an ECDSA signature on an MD5SHA1 hash payload
    EcdsaSignMd5sha1 = 0x12,
    // requests an ECDSA signature on an SHA1 hash payload
    EcdsaSignSha1 = 0x13,
    // requests an ECDSA signature on an SHA224 hash payload
    EcdsaSignSha224 = 0x14,
    // requests an ECDSA signature on an SHA256 hash payload
    EcdsaSignSha256 = 0x15,
    // requests an ECDSA signature on an SHA384 hash payload
    EcdsaSignSha384 = 0x16,
    // requests an ECDSA signature on an SHA512 hash payload
    EcdsaSignSha512 = 0x17,
    // requests an Ed25519 signature on an arbitrary-length payload
    Ed25519Sign = 0x18,
    // asks to encrypt a blob (like a Session Ticket)
    #[allow(unused)]
    Seal = 0x21,
    // asks to decrypt a blob encrypted by OpSeal
    #[allow(unused)]
    Unseal = 0x22,
    // requests an RSASSA-PSS signature on an SHA256 hash payload
    RsaPssSignSha256 = 0x35,
    // requests an RSASSA-PSS signature on an SHA384 hash payload
    RsaPssSignSha384 = 0x36,
    // requests an RSASSA-PSS signature on an SHA512 hash payload
    RsaPssSignSha512 = 0x37,
}

impl TryFrom<KeylessAction> for KeylessOpCode {
    type Error = anyhow::Error;

    fn try_from(value: KeylessAction) -> Result<Self, Self::Error> {
        match value {
            KeylessAction::RsaPrivateDecrypt(KeylessRsaPadding::Pkcs1) => {
                Ok(KeylessOpCode::RsaDecrypt)
            }
            KeylessAction::RsaPrivateDecrypt(padding) => {
                Err(anyhow!("unsupported rsa padding type {padding:?}"))
            }
            KeylessAction::RsaSign(KeylessSignDigest::Md5Sha1, KeylessRsaPadding::Pkcs1) => {
                Ok(KeylessOpCode::RsaSignMd5Sha1)
            }
            KeylessAction::RsaSign(KeylessSignDigest::Sha1, KeylessRsaPadding::Pkcs1) => {
                Ok(KeylessOpCode::RsaSignSha1)
            }
            KeylessAction::RsaSign(KeylessSignDigest::Sha224, KeylessRsaPadding::Pkcs1) => {
                Ok(KeylessOpCode::RsaSignSha224)
            }
            KeylessAction::RsaSign(KeylessSignDigest::Sha256, KeylessRsaPadding::Pkcs1) => {
                Ok(KeylessOpCode::RsaSignSha256)
            }
            KeylessAction::RsaSign(KeylessSignDigest::Sha256, KeylessRsaPadding::Pss) => {
                Ok(KeylessOpCode::RsaPssSignSha256)
            }
            KeylessAction::RsaSign(KeylessSignDigest::Sha384, KeylessRsaPadding::Pkcs1) => {
                Ok(KeylessOpCode::RsaSignSha384)
            }
            KeylessAction::RsaSign(KeylessSignDigest::Sha384, KeylessRsaPadding::Pss) => {
                Ok(KeylessOpCode::RsaPssSignSha384)
            }
            KeylessAction::RsaSign(KeylessSignDigest::Sha512, KeylessRsaPadding::Pkcs1) => {
                Ok(KeylessOpCode::RsaSignSha512)
            }
            KeylessAction::RsaSign(KeylessSignDigest::Sha512, KeylessRsaPadding::Pss) => {
                Ok(KeylessOpCode::RsaPssSignSha512)
            }
            KeylessAction::RsaSign(digest, padding) => Err(anyhow!(
                "unsupported rsa sign using digest {digest:?} padding {padding:?}"
            )),
            KeylessAction::EcdsaSign(KeylessSignDigest::Md5Sha1) => {
                Ok(KeylessOpCode::EcdsaSignMd5sha1)
            }
            KeylessAction::EcdsaSign(KeylessSignDigest::Sha1) => Ok(KeylessOpCode::EcdsaSignSha1),
            KeylessAction::EcdsaSign(KeylessSignDigest::Sha224) => {
                Ok(KeylessOpCode::EcdsaSignSha224)
            }
            KeylessAction::EcdsaSign(KeylessSignDigest::Sha256) => {
                Ok(KeylessOpCode::EcdsaSignSha256)
            }
            KeylessAction::EcdsaSign(KeylessSignDigest::Sha384) => {
                Ok(KeylessOpCode::EcdsaSignSha384)
            }
            KeylessAction::EcdsaSign(KeylessSignDigest::Sha512) => {
                Ok(KeylessOpCode::EcdsaSignSha512)
            }
            KeylessAction::Ed25519Sign => Ok(KeylessOpCode::Ed25519Sign),
            _ => Err(anyhow!("unsupported action: {value:?}")),
        }
    }
}

pub(crate) struct KeylessRequestBuilder {
    opcode: KeylessOpCode,
    cert_digest: DigestBytes,
    server_name: String,
}

impl KeylessRequestBuilder {
    pub(crate) fn new(digest: DigestBytes, action: KeylessAction) -> anyhow::Result<Self> {
        let opcode = KeylessOpCode::try_from(action)?;
        Ok(KeylessRequestBuilder {
            opcode,
            cert_digest: digest,
            server_name: String::new(),
        })
    }

    pub(crate) fn build(&self, payload: &[u8]) -> anyhow::Result<KeylessRequest> {
        let mut buf = Vec::with_capacity(512);
        // hdr and ID
        buf.extend_from_slice(&[0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);

        // certificate digest
        buf.push(0x01);
        let digest_len = self.cert_digest.len();
        buf.push(((digest_len >> 8) & 0xFF) as u8);
        buf.push((digest_len & 0xFF) as u8);
        buf.put_slice(self.cert_digest.as_ref());

        if !self.server_name.is_empty() {
            // TODO
        }

        // OpCode
        buf.put_slice(&[0x11, 0x00, 0x01]);
        buf.push(self.opcode as u8);

        // Payload
        buf.push(0x12);
        let payload_len = payload.len();
        if payload_len > u16::MAX as usize {
            return Err(anyhow!("payload length too long"));
        }
        buf.push(((payload_len >> 8) & 0xFF) as u8);
        buf.push((payload_len & 0xFF) as u8);
        buf.put_slice(&payload[0..payload_len]);

        match super::MESSAGE_PADDED_LENGTH.checked_sub(buf.len()) {
            Some(0) => {}
            Some(1..=super::ITEM_HEADER_LENGTH) => buf.put_slice(&[0x20, 0x00, 0x00]),
            Some(n) => {
                let left = n - super::ITEM_HEADER_LENGTH;
                buf.push(0x20);
                buf.push(((left >> 8) & 0xFF) as u8);
                buf.push((left & 0xFF) as u8);
                buf.resize(super::MESSAGE_PADDED_LENGTH, 0);
            }
            None => {}
        }

        let len = buf.len() - super::MESSAGE_HEADER_LENGTH;
        if len > u16::MAX as usize {
            return Err(anyhow!("message length too long"));
        }
        buf[2] = ((len >> 8) & 0xFF) as u8;
        buf[3] = (len & 0xFF) as u8;

        Ok(KeylessRequest { buf, id: 0 })
    }
}

#[derive(Clone)]
pub(crate) struct KeylessRequest {
    buf: Vec<u8>,
    id: u32,
}

impl KeylessRequest {
    pub(crate) fn set_id(&mut self, id: u32) {
        let b = id.to_be_bytes();
        self.buf[4] = b[0];
        self.buf[5] = b[1];
        self.buf[6] = b[2];
        self.buf[7] = b[3];
        self.id = id;
    }

    #[inline]
    pub(crate) fn id(&self) -> u32 {
        self.id
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        self.buf.as_slice()
    }
}
