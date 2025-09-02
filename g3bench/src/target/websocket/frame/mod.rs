/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum FrameType {
    Continue = 0x0,
    Text = 0x1,
    Binary = 0x2,
    Close = 0x8,
    Ping = 0x9,
    Pong = 0xA,
}

impl FrameType {
    fn as_str(&self) -> &'static str {
        match self {
            FrameType::Continue => "Continue",
            FrameType::Text => "Text",
            FrameType::Binary => "Binary",
            FrameType::Close => "Close",
            FrameType::Ping => "Ping",
            FrameType::Pong => "Pong",
        }
    }
}

impl fmt::Display for FrameType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<u8> for FrameType {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value & 0x0F {
            0x0 => Ok(FrameType::Continue),
            0x1 => Ok(FrameType::Text),
            0x2 => Ok(FrameType::Binary),
            0x8 => Ok(FrameType::Close),
            0x9 => Ok(FrameType::Ping),
            0xA => Ok(FrameType::Pong),
            n => Err(n),
        }
    }
}

mod server;
pub(super) use server::ServerFrameHeader;

mod client;
pub(super) use client::ClientFrameBuilder;
