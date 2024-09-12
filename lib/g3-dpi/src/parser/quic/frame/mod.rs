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

mod crypto;
pub use crypto::{ClientHelloConsumer, CryptoFrame};

#[derive(Debug, Error)]
pub enum FrameParseError {
    #[error("invalid frame type {0}")]
    InvalidFrameType(u64),
    #[error("no enough data")]
    NoEnoughData,
    #[error("too bug offset value {0}")]
    TooBigOffsetValue(u64),
    #[error("out of order frame: {0}")]
    OutOfOrderFrame(&'static str),
    #[error("malformed frame: {0}")]
    MalformedFrame(&'static str),
}

pub trait FrameConsume {
    fn recv_crypto(&mut self, frame: &CryptoFrame<'_>) -> Result<(), FrameParseError>;
}
