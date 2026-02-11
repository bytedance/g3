/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

mod packet;
pub use packet::{InitialPacket, PacketParseError};

mod message;
pub use message::HandshakeCoalescer;

#[cfg(test)]
mod tests;
