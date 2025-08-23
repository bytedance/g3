/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use thiserror::Error;

mod crypto;
pub use crypto::{CryptoFrame, HandshakeCoalescer};

mod ack;
pub use ack::AckFrame;

#[derive(Debug, Error)]
pub enum FrameParseError {
    #[error("invalid frame type {0}")]
    InvalidFrameType(u64),
    #[error("not enough data")]
    NotEnoughData,
    #[error("too big offset value {0}")]
    TooBigOffsetValue(u64),
    #[error("out of order frame: {0}")]
    OutOfOrderFrame(&'static str),
    #[error("malformed frame: {0}")]
    MalformedFrame(&'static str),
}

pub trait FrameConsume {
    fn recv_crypto(&mut self, frame: &CryptoFrame<'_>) -> Result<(), FrameParseError>;
}
