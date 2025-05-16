/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use thiserror::Error;

const BUF_PREFIX_LEN: usize =
    super::MESSAGE_HEADER_LENGTH + super::ITEM_HEADER_LENGTH + 1 + super::ITEM_HEADER_LENGTH;

pub(crate) struct KeylessPongResponse {
    pub(crate) id: u32,
    pub(crate) buf: Vec<u8>,
}

impl KeylessPongResponse {
    pub(crate) fn new(id: u32, payload: &[u8]) -> Self {
        let item_len = payload.len() as u16;
        let item_len_h = (item_len >> 8) as u8;
        let item_len_l = (item_len & 0xFF) as u8;

        let msg_len = (payload.len() + BUF_PREFIX_LEN - super::MESSAGE_HEADER_LENGTH) as u16;
        let msg_len_h = (msg_len >> 8) as u8;
        let msg_len_l = (msg_len & 0xFF) as u8;

        let b = id.to_be_bytes();
        let prefix: [u8; BUF_PREFIX_LEN] = [
            0x01, 0x00, // protocol version
            msg_len_h, msg_len_l, // message length
            b[0], b[1], b[2], b[3], // message id
            0x11, 0x00, 0x01, 0xF2, // OpCode
            0x12, item_len_h, item_len_l, // Payload
        ];
        let mut buf = Vec::with_capacity(payload.len() + BUF_PREFIX_LEN);
        buf.extend_from_slice(&prefix);
        buf.extend_from_slice(payload);

        KeylessPongResponse { id, buf }
    }
}

pub(crate) struct KeylessDataResponse {
    pub(crate) id: u32,
    pub(crate) buf: Vec<u8>,
}

impl KeylessDataResponse {
    pub(crate) fn new(id: u32, key_size: usize) -> Self {
        let b = id.to_be_bytes();
        let prefix: [u8; BUF_PREFIX_LEN] = [
            0x01, 0x00, // protocol version
            0x00, 0x00, // message length
            b[0], b[1], b[2], b[3], // message id
            0x11, 0x00, 0x01, 0xF0, // OpCode
            0x12, 0x00, 0x00, // Payload
        ];
        let buf_max_size = prefix.len() + key_size;
        let mut buf = Vec::with_capacity(buf_max_size);
        buf.extend_from_slice(&prefix);
        unsafe { buf.set_len(buf_max_size) };
        KeylessDataResponse { id, buf }
    }

    pub(crate) fn payload_data_mut(&mut self) -> &mut [u8] {
        &mut self.buf[BUF_PREFIX_LEN..]
    }

    pub(crate) fn finalize_payload(&mut self, payload_len: usize) {
        let buf_len = payload_len + BUF_PREFIX_LEN;
        unsafe { self.buf.set_len(buf_len) };

        let item_len = payload_len as u16;
        self.buf[13] = (item_len >> 8) as u8;
        self.buf[14] = (item_len & 0xFF) as u8;

        let msg_len = (buf_len - super::MESSAGE_HEADER_LENGTH) as u16;
        self.buf[2] = (msg_len >> 8) as u8;
        self.buf[3] = (msg_len & 0xFF) as u8;
    }
}

#[derive(Clone, Copy, Debug, Error)]
#[repr(u8)]
pub(crate) enum KeylessResponseErrorCode {
    #[error("no error")]
    NoError = 0,
    #[error("cryptography failure")]
    CryptographyFailure = 1,
    #[error("no matching certificate ID")]
    KeyNotFound = 2,
    #[error("I/O read failure")]
    ReadError = 3,
    #[error("unsupported version incorrect")]
    VersionMismatch = 4,
    #[error("use of unknown opcode in request")]
    BadOpCode = 5,
    #[error("use of unexpected opcode in request")]
    UnexpectedOpCode = 6,
    #[error("malformed message")]
    FormatError = 7,
    #[error("memory or other internal error")]
    InternalError = 8,
    #[error("certificate not found")]
    CertNotFound = 9,
    #[error("sealing key expired")]
    Expired = 10,
    #[error("the remote keyserver was not configured correctly")]
    RemoteConfiguration = 11,
}

#[derive(Clone, Copy)]
pub(crate) struct KeylessErrorResponse {
    pub(crate) id: u32,
    pub(crate) buf: [u8; BUF_PREFIX_LEN + 1],
}

impl KeylessErrorResponse {
    pub(crate) fn new(id: u32) -> Self {
        let b = id.to_be_bytes();
        KeylessErrorResponse {
            id,
            buf: [
                0x01, 0x00, // protocol version
                0x00, 0x08, // message length
                b[0], b[1], b[2], b[3], // message id
                0x11, 0x00, 0x01, 0xFF, // OpCode
                0x12, 0x00, 0x01, 0x00, // Payload
            ],
        }
    }

    pub(crate) fn error_code(&self) -> KeylessResponseErrorCode {
        match self.buf[BUF_PREFIX_LEN] {
            0 => KeylessResponseErrorCode::NoError,
            1 => KeylessResponseErrorCode::CryptographyFailure,
            2 => KeylessResponseErrorCode::KeyNotFound,
            3 => KeylessResponseErrorCode::ReadError,
            4 => KeylessResponseErrorCode::VersionMismatch,
            5 => KeylessResponseErrorCode::BadOpCode,
            6 => KeylessResponseErrorCode::UnexpectedOpCode,
            7 => KeylessResponseErrorCode::FormatError,
            8 => KeylessResponseErrorCode::InternalError,
            9 => KeylessResponseErrorCode::CertNotFound,
            10 => KeylessResponseErrorCode::Expired,
            11 => KeylessResponseErrorCode::RemoteConfiguration,
            _ => unreachable!(),
        }
    }

    fn set_error_code(mut self, error_code: KeylessResponseErrorCode) -> Self {
        self.buf[BUF_PREFIX_LEN] = error_code as u8;
        self
    }

    #[inline]
    pub(crate) fn key_not_found(self) -> Self {
        self.set_error_code(KeylessResponseErrorCode::KeyNotFound)
    }

    #[inline]
    pub(crate) fn bad_op_code(self) -> Self {
        self.set_error_code(KeylessResponseErrorCode::BadOpCode)
    }

    #[inline]
    pub(crate) fn unexpected_op_code(self) -> Self {
        self.set_error_code(KeylessResponseErrorCode::UnexpectedOpCode)
    }

    #[inline]
    pub(crate) fn crypto_fail(self) -> Self {
        self.set_error_code(KeylessResponseErrorCode::CryptographyFailure)
    }

    #[inline]
    pub(crate) fn format_error(self) -> Self {
        self.set_error_code(KeylessResponseErrorCode::FormatError)
    }
}

pub(crate) enum KeylessResponse {
    Data(KeylessDataResponse),
    Pong(KeylessPongResponse),
    Error(KeylessErrorResponse),
}

impl KeylessResponse {
    pub(crate) fn message(&self) -> &[u8] {
        match self {
            KeylessResponse::Data(d) => &d.buf,
            KeylessResponse::Pong(p) => &p.buf,
            KeylessResponse::Error(e) => &e.buf,
        }
    }

    #[allow(unused)]
    pub(crate) fn id(&self) -> u32 {
        match self {
            KeylessResponse::Data(d) => d.id,
            KeylessResponse::Pong(p) => p.id,
            KeylessResponse::Error(e) => e.id,
        }
    }
}
