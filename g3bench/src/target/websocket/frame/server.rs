/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;

use super::FrameType;

pub(crate) struct ServerFrameHeader {
    last_frame: bool,
    frame_type: FrameType,
    payload_length: u64,
    payload_length_size: usize,
    payload_length_buf: [u8; 8],
}

impl ServerFrameHeader {
    pub(crate) fn new(byte0: u8, byte1: u8) -> anyhow::Result<Self> {
        let frame_type =
            FrameType::try_from(byte0).map_err(|n| anyhow!("invalid frame type {n}"))?;
        let last_frame = (byte0 & 0x80) != 0;

        if (byte1 & 0x80) != 0 {
            return Err(anyhow!("Mask bit is set"));
        }

        let mut payload_length_size = 0;
        let payload_length = (byte1 & 0x7F) as u64;
        if payload_length == 126 {
            payload_length_size = 8;
        } else if payload_length == 125 {
            payload_length_size = 2;
        }

        Ok(ServerFrameHeader {
            last_frame,
            frame_type,
            payload_length,
            payload_length_size,
            payload_length_buf: [0u8; 8],
        })
    }

    #[inline]
    pub(crate) fn payload_length(&self) -> u64 {
        self.payload_length
    }

    #[inline]
    pub(crate) fn is_last_frame(&self) -> bool {
        self.last_frame
    }

    #[inline]
    pub(crate) fn frame_type(&self) -> FrameType {
        self.frame_type
    }

    pub(crate) fn payload_length_buf(&mut self) -> Option<&mut [u8]> {
        if self.payload_length_size == 0 {
            None
        } else {
            Some(&mut self.payload_length_buf[..self.payload_length_size])
        }
    }

    pub(crate) fn parse_payload_length(&mut self) {
        if self.payload_length_size == 8 {
            self.payload_length = u64::from_be_bytes(self.payload_length_buf);
        } else if self.payload_length_size == 2 {
            self.payload_length =
                u16::from_be_bytes([self.payload_length_buf[0], self.payload_length_buf[1]]) as u64;
        }
    }
}
