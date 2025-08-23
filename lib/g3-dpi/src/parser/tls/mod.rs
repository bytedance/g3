/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawVersion {
    major: u8,
    minor: u8,
}

impl RawVersion {
    pub fn is_tlcp(&self) -> bool {
        (self.major == 1) && (self.minor == 1)
    }
}

mod record;
pub(crate) use record::ContentType;
pub use record::{Record, RecordHeader, RecordParseError};

mod handshake;
#[cfg(feature = "quic")]
pub(crate) use handshake::HandshakeHeader;
pub(crate) use handshake::HandshakeType;
pub use handshake::{ClientHello, ClientHelloParseError, HandshakeCoalescer, HandshakeMessage};

mod extension;
pub use extension::{ExtensionList, ExtensionParseError, ExtensionType};

#[cfg(test)]
mod tests;
