/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

pub enum LogDropType {
    FormatFailed,
    ChannelClosed,
    ChannelOverflow,
    PeerUnreachable,
}

impl LogDropType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            LogDropType::FormatFailed => "FormatFailed",
            LogDropType::ChannelClosed => "ChannelClosed",
            LogDropType::ChannelOverflow => "ChannelOverflow",
            LogDropType::PeerUnreachable => "PeerUnreachable",
        }
    }
}

impl AsRef<str> for LogDropType {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
