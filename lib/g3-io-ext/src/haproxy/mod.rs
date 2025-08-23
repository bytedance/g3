/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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
