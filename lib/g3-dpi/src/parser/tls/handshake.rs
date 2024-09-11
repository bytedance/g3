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

use thiserror::Error;

#[allow(dead_code)]
#[repr(u8)]
pub enum HandshakeType {
    HelloRequestReserved = 0,
    ClientHello = 1,
    ServerHell0 = 2,
    HelloVerifyRequestReserved = 3,
    // there are more that we don't need
}

#[derive(Debug, Error)]
pub enum HandshakeParseError {
    #[error("invalid data size {0} for TLS record header")]
    InvalidDataSize(usize),
}

pub struct HandshakeHeader {
    pub msg_type: u8,
    pub msg_length: u32,
}

impl HandshakeHeader {
    pub const SIZE: usize = 4;

    pub fn parse(data: &[u8]) -> Result<Self, HandshakeParseError> {
        if data.len() < Self::SIZE {
            return Err(HandshakeParseError::InvalidDataSize(data.len()));
        }

        Ok(HandshakeHeader {
            msg_type: data[0],
            msg_length: u32::from_be_bytes([0, data[1], data[2], data[3]]),
        })
    }
}
