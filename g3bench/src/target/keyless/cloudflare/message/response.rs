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

use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};

#[derive(Clone, Copy, Debug, Error)]
pub(crate) enum KeylessServerError {
    #[error("cryptography error")]
    CryptographyFailure,
    #[error("key not found due to no matching SKI/SNI/ServerIP")]
    KeyNotFound,
    #[error("I/O read failure")]
    ReadError,
    #[error("version mismatch")]
    VersionMismatch,
    #[error("bad opcode")]
    BadOpCode,
    #[error("unexpected opcode")]
    UnexpectedOpCode,
    #[error("malformed message")]
    FormatError,
    #[error("internal error")]
    InternalError,
    #[error("certificate not found")]
    CertNotFound,
    #[error("sealing key expired")]
    Expired,
}

impl From<u8> for KeylessResponseError {
    fn from(value: u8) -> Self {
        match value {
            0x01 => KeylessServerError::CryptographyFailure.into(),
            0x02 => KeylessServerError::KeyNotFound.into(),
            0x03 => KeylessServerError::ReadError.into(),
            0x04 => KeylessServerError::VersionMismatch.into(),
            0x05 => KeylessServerError::BadOpCode.into(),
            0x06 => KeylessServerError::UnexpectedOpCode.into(),
            0x07 => KeylessServerError::FormatError.into(),
            0x08 => KeylessServerError::InternalError.into(),
            0x09 => KeylessServerError::CertNotFound.into(),
            0x0A => KeylessServerError::Expired.into(),
            n => KeylessLocalError::UnsupportedServerErrorCode(n).into(),
        }
    }
}

#[derive(Debug, Error)]
pub(crate) enum KeylessLocalError {
    #[error("invalid message length")]
    InvalidMessageLength,
    #[error("unexpected version {0}.{1}")]
    UnexpectedVersion(u8, u8),
    #[error("read failed: {0:?}")]
    ReadFailed(io::Error),
    #[error("write failed: {0:?}")]
    WriteFailed(io::Error),
    #[error("not enough data for a valid item")]
    NotEnoughData,
    #[error("too long item length")]
    TooLongItemLength,
    #[error("invalid length for item {0}")]
    InvalidItemLength(u8),
    #[error("invalid item tag {0}")]
    InvalidItemTag(u8),
    #[error("invalid opcode {0}")]
    InvalidOpCode(u8),
    #[error("unsupported server error code {0}")]
    UnsupportedServerErrorCode(u8),
}

#[derive(Debug, Error)]
pub(crate) enum KeylessResponseError {
    #[error("server error: {0}")]
    ServerError(#[from] KeylessServerError),
    #[error("local error: {0}")]
    LocalError(#[from] KeylessLocalError),
}

fn parse_buf(buf: &[u8]) -> Result<Vec<u8>, KeylessResponseError> {
    let total_len = buf.len();
    let mut opcode = 0;
    let mut data_buf = buf;

    let mut offset = 0usize;
    loop {
        if offset + super::ITEM_HEADER_LENGTH > total_len {
            return Err(KeylessLocalError::NotEnoughData.into());
        }

        let hdr = &buf[offset..offset + super::ITEM_HEADER_LENGTH];
        let item = hdr[0];
        let item_len = ((hdr[1] as usize) << 8) + hdr[2] as usize;
        if item_len < 1 {
            return Err(KeylessLocalError::InvalidItemLength(item).into());
        }
        offset += super::ITEM_HEADER_LENGTH;
        if offset + item_len > total_len {
            return Err(KeylessLocalError::TooLongItemLength.into());
        }

        let data = &buf[offset..offset + item_len];
        match item {
            // OPCODE
            0x11 => {
                if item_len != 1 {
                    return Err(KeylessLocalError::InvalidItemLength(item).into());
                }
                opcode = data[0];
            }
            // PAYLOAD
            0x12 => data_buf = data,
            // PADDING
            0x20 => {}
            _ => return Err(KeylessLocalError::InvalidItemTag(item).into()),
        }

        offset += item_len;
        if offset >= total_len {
            break;
        }
    }

    match opcode {
        0xF0 => Ok(data_buf.to_vec()),
        0xFF => {
            if data_buf.len() != 1 {
                return Err(KeylessLocalError::InvalidItemLength(0x12).into());
            }
            Err(KeylessResponseError::from(data_buf[0]))
        }
        _ => Err(KeylessLocalError::InvalidOpCode(opcode).into()),
    }
}

pub(crate) struct KeylessResponse {
    id: u32,
    #[allow(unused)]
    data: Vec<u8>,
}

impl KeylessResponse {
    #[inline]
    pub(crate) fn id(&self) -> u32 {
        self.id
    }

    pub(crate) async fn read<R>(
        reader: &mut R,
        buf: &mut Vec<u8>,
    ) -> Result<Self, KeylessResponseError>
    where
        R: AsyncRead + Unpin,
    {
        let mut hdr_buf = [0u8; 8];
        let len = reader
            .read_exact(&mut hdr_buf)
            .await
            .map_err(KeylessLocalError::ReadFailed)?;
        if len < 4 {
            return Err(KeylessLocalError::InvalidMessageLength.into());
        }

        let major = hdr_buf[0];
        let minor = hdr_buf[1];
        if major != 1 || minor != 0 {
            return Err(KeylessLocalError::UnexpectedVersion(major, minor).into());
        }

        let len = ((hdr_buf[2] as usize) << 8) + hdr_buf[3] as usize;
        buf.clear();
        buf.resize(len, 0);
        let nr = reader
            .read_exact(buf)
            .await
            .map_err(KeylessLocalError::ReadFailed)?;
        if nr < len {
            return Err(KeylessLocalError::InvalidMessageLength.into());
        }

        let id = u32::from_be_bytes([hdr_buf[4], hdr_buf[5], hdr_buf[6], hdr_buf[7]]);
        let data = parse_buf(buf)?;

        Ok(KeylessResponse { id, data })
    }
}
