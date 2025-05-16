/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

mod var_int;
pub use var_int::VarInt;

mod packet;
pub use packet::{InitialPacket, PacketParseError};

mod frame;
pub use frame::{AckFrame, CryptoFrame, FrameConsume, FrameParseError, HandshakeCoalescer};

#[cfg(test)]
mod tests;
