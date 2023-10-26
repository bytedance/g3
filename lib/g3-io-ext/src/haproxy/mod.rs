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
use std::net::SocketAddr;

use thiserror::Error;

mod v1;
pub use v1::ProxyProtocolV1Reader;

mod v2;
pub use v2::ProxyProtocolV2Reader;

pub struct ProxyAddr {
    pub src_addr: SocketAddr,
    pub dst_addr: SocketAddr,
}

#[derive(Debug, Error)]
pub enum ProxyProtocolReadError {
    #[error("read failed: {0:?}")]
    ReadFailed(#[from] io::Error),
    #[error("close unexpected")]
    ClosedUnexpected,
    #[error("read timed out")]
    ReadTimeout,
    #[error("invalid magic header")]
    InvalidMagicHeader,
    #[error("invalid version: {0}")]
    InvalidVersion(u8),
    #[error("invalid command: {0}")]
    InvalidCommand(u8),
    #[error("invalid family: {0}")]
    InvalidFamily(u8),
    #[error("invalid protocol: {0}")]
    InvalidProtocol(u8),
    #[error("invalid data length {0}")]
    InvalidDataLength(usize),
    #[error("invalid src address")]
    InvalidSrcAddr,
    #[error("invalid dst address")]
    InvalidDstAddr,
}
