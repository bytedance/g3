/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use super::FrameType;

pub(crate) struct ClientFrameBuilder {
    frame_type: FrameType,
    max_frame_size: usize,
}

impl ClientFrameBuilder {
    pub(crate) fn new(frame_type: FrameType, max_frame_size: usize) -> Self {
        ClientFrameBuilder {
            frame_type,
            max_frame_size,
        }
    }

    fn build_request_frame(&self, frame_type: u8, data: &[u8], buf: &mut Vec<u8>) {
        buf.push(frame_type);

        let payload_len = data.len();
        if payload_len > u16::MAX as usize {
            let bytes = (payload_len as u64).to_be_bytes();
            buf.push(0b1111_1111);
            buf.extend_from_slice(&bytes);
        } else if payload_len > 125 {
            let bytes = u16::try_from(payload_len).unwrap().to_be_bytes();
            buf.push(0b1111_1110);
            buf.extend_from_slice(&bytes);
        } else {
            buf.push(u8::try_from(payload_len).unwrap() & 0b1000_0000);
        }

        let mut mask = [0u8; 4];
        fastrand::fill(&mut mask);
        buf.extend_from_slice(&mask);

        if !data.is_empty() {
            buf.reserve(data.len());
            // TODO use SIMD XOR
            let mut chunks_iter = data.chunks_exact(4);
            for s in &mut chunks_iter {
                buf.push(s[0] ^ mask[0]);
                buf.push(s[1] ^ mask[1]);
                buf.push(s[2] ^ mask[2]);
                buf.push(s[3] ^ mask[3]);
            }
            for (i, b) in chunks_iter.remainder().iter().enumerate() {
                buf.push(*b ^ mask[i]);
            }
        }
    }

    pub(crate) fn build_frames(&self, data: &[u8], buf: &mut Vec<u8>) {
        let mut chunks_iter = data.chunks_exact(self.max_frame_size);
        let frame_type: u8 = self.frame_type as u8;
        if let Some(chunk) = chunks_iter.next() {
            self.build_request_frame(frame_type, chunk, buf);

            let mut last_frame_offset = 0usize;
            for chunk in &mut chunks_iter {
                last_frame_offset = buf.len();
                self.build_request_frame(0x00, chunk, buf);
            }

            if chunks_iter.remainder().is_empty() {
                buf[last_frame_offset] |= 0x80; // Set FIN bit
            } else {
                self.build_request_frame(0x80, chunks_iter.remainder(), buf);
            }
        } else {
            self.build_request_frame(frame_type | 0x80, chunks_iter.remainder(), buf);
        }
    }
}
