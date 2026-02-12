/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

mod var_int;
pub use var_int::VarInt;

mod frame;
pub use frame::{AckFrame, AckRange, CryptoFrame, EcnCounts, FrameConsume, FrameParseError};

mod packet;
pub use packet::{InitialPacket, PacketParseError};

mod message;
pub use message::HandshakeCoalescer;

#[cfg(test)]
mod tests;
