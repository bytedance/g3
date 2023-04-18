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
use openssl::hash::{DigestBytes, MessageDigest};
use openssl::x509::X509Ref;

use crate::target::keyless::opts::{KeylessAction, KeylessRsaPadding, KeylessSignDigest};

#[non_exhaustive]
#[repr(u8)]
#[derive(Clone, Copy)]
pub(crate) enum KeylessOpCode {
    RsaDecrypt = 0x01,
    RsaSignMd5Sha1 = 0x02,
    RsaSignSha1 = 0x03,
    RsaSignSha224 = 0x04,
    RsaSignSha256 = 0x05,
    RsaSignSha384 = 0x06,
    RsaSignSha512 = 0x07,
    RsaRawDecrypt = 0x08,
    #[allow(unused)]
    EcdsaMask = 0x10,
    EcdsaSignMd5sha1 = 0x12,
    EcdsaSignSha1 = 0x13,
    EcdsaSignSha224 = 0x14,
    EcdsaSignSha256 = 0x15,
    EcdsaSignSha384 = 0x16,
    EcdsaSignSha512 = 0x17,
}

impl TryFrom<KeylessAction> for KeylessOpCode {
    type Error = anyhow::Error;

    fn try_from(value: KeylessAction) -> Result<Self, Self::Error> {
        match value {
            KeylessAction::RsaPrivateDecrypt(KeylessRsaPadding::None) => {
                Ok(KeylessOpCode::RsaRawDecrypt)
            }
            KeylessAction::RsaPrivateDecrypt(KeylessRsaPadding::Pkcs1) => {
                Ok(KeylessOpCode::RsaDecrypt)
            }
            KeylessAction::RsaPrivateDecrypt(padding) => {
                Err(anyhow!("unsupported rsa padding type {padding:?}"))
            }
            KeylessAction::RsaSign(KeylessSignDigest::Md5Sha1) => Ok(KeylessOpCode::RsaSignMd5Sha1),
            KeylessAction::RsaSign(KeylessSignDigest::Sha1) => Ok(KeylessOpCode::RsaSignSha1),
            KeylessAction::RsaSign(KeylessSignDigest::Sha224) => Ok(KeylessOpCode::RsaSignSha224),
            KeylessAction::RsaSign(KeylessSignDigest::Sha256) => Ok(KeylessOpCode::RsaSignSha256),
            KeylessAction::RsaSign(KeylessSignDigest::Sha384) => Ok(KeylessOpCode::RsaSignSha384),
            KeylessAction::RsaSign(KeylessSignDigest::Sha512) => Ok(KeylessOpCode::RsaSignSha512),
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
    pub(crate) fn new(cert: &X509Ref, action: KeylessAction) -> anyhow::Result<Self> {
        let opcode = KeylessOpCode::try_from(action)?;
        let digest = cert
            .digest(MessageDigest::sha256())
            .map_err(|e| anyhow!("failed to get cert digest: {e}"))?;
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
        buf.push(0x11);
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

        let len = buf.len() - 4;
        if len > u16::MAX as usize {
            return Err(anyhow!("message length too long"));
        }

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
