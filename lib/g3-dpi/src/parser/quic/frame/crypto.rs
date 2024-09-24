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

use std::cmp::Ordering;
use std::collections::BTreeSet;

use super::{FrameConsume, FrameParseError};
use crate::parser::quic::VarInt;
use crate::parser::tls::{ClientHello, ClientHelloParseError, HandshakeHeader, HandshakeType};

pub struct CryptoFrame<'a> {
    pub stream_offset: usize,
    pub data: &'a [u8],
    pub(crate) encoded_len: usize,
}

impl<'a> CryptoFrame<'a> {
    /// Parse a Crypto Frame from a packet buffer
    pub fn parse(data: &'a [u8]) -> Result<Self, FrameParseError> {
        let Some(stream_offset) = VarInt::try_parse(data) else {
            return Err(FrameParseError::NotEnoughData);
        };
        let mut offset = stream_offset.encoded_len();
        let stream_offset = usize::try_from(stream_offset.value())
            .map_err(|_| FrameParseError::TooBigOffsetValue(stream_offset.value()))?;

        let left = &data[offset..];
        let Some(length) = VarInt::try_parse(left) else {
            return Err(FrameParseError::NotEnoughData);
        };
        offset += length.encoded_len();

        if offset as u64 + length.value() > data.len() as u64 {
            return Err(FrameParseError::NotEnoughData);
        }

        let data_end = offset + length.value() as usize;
        Ok(CryptoFrame {
            stream_offset,
            data: &data[offset..data_end],
            encoded_len: data_end,
        })
    }
}

#[derive(PartialEq, Eq)]
struct Fragment {
    offset: usize,
    length: usize,
}

impl PartialOrd for Fragment {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.offset.partial_cmp(&other.offset) {
            Some(Ordering::Greater) => Some(Ordering::Greater),
            Some(Ordering::Less) => Some(Ordering::Less),
            Some(Ordering::Equal) => self.length.partial_cmp(&other.length),
            None => None,
        }
    }
}

impl Ord for Fragment {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.offset.cmp(&other.offset) {
            Ordering::Greater => Ordering::Greater,
            Ordering::Less => Ordering::Less,
            Ordering::Equal => self.length.cmp(&other.length),
        }
    }
}

pub struct HandshakeCoalescer {
    max_message_size: u32,
    header: Option<HandshakeHeader>,
    buf: Vec<u8>,
    unfilled_offset: usize,
    expected_length: usize,
    fragment_set: BTreeSet<Fragment>,
}

impl Default for HandshakeCoalescer {
    fn default() -> Self {
        Self::new(1 << 14)
    }
}

impl HandshakeCoalescer {
    pub fn new(max_message_size: u32) -> Self {
        HandshakeCoalescer {
            max_message_size,
            header: None,
            buf: Vec::with_capacity(1024),
            unfilled_offset: 0,
            expected_length: 0,
            fragment_set: BTreeSet::new(),
        }
    }

    pub(crate) fn finished(&self) -> bool {
        self.expected_length > 0 && self.expected_length == self.unfilled_offset
    }

    /// Parse this message as a ClientHello message
    pub fn parse_client_hello(&self) -> Result<Option<ClientHello<'_>>, ClientHelloParseError> {
        let Some(hdr) = &self.header else {
            return Ok(None);
        };
        if hdr.msg_type != HandshakeType::ClientHello as u8 {
            return Err(ClientHelloParseError::InvalidMessageType(hdr.msg_type));
        }
        if self.finished() {
            let ch = ClientHello::parse_msg_data(&self.buf[HandshakeHeader::SIZE..])?;
            Ok(Some(ch))
        } else {
            Ok(None)
        }
    }
}

impl FrameConsume for HandshakeCoalescer {
    fn recv_crypto(&mut self, frame: &CryptoFrame<'_>) -> Result<(), FrameParseError> {
        if frame.stream_offset <= self.unfilled_offset {
            let frame_stream_end = frame.stream_offset + frame.data.len();
            if self.expected_length > 0 && frame_stream_end > self.expected_length {
                // invalid frames
                return Err(FrameParseError::MalformedFrame(
                    "more data than expected in crypto frame",
                ));
            }

            if self.buf.len() > frame_stream_end {
                // write part of the buf
                let dst = &mut self.buf[frame.stream_offset..frame_stream_end];
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        frame.data.as_ptr(),
                        dst.as_mut_ptr(),
                        frame.data.len(),
                    );
                }

                if frame_stream_end > self.unfilled_offset {
                    // we have some new data filled in
                    self.unfilled_offset = frame_stream_end;

                    if !self.fragment_set.is_empty() {
                        // TODO use BTreeSet::extract_if after stablized
                        for fragment in &self.fragment_set {
                            if fragment.offset > self.unfilled_offset {
                                break;
                            }
                            let fragment_end = fragment.offset + fragment.length;
                            if fragment_end > self.unfilled_offset {
                                self.unfilled_offset = fragment_end;
                            }
                        }

                        self.fragment_set
                            .retain(|v| v.offset > self.unfilled_offset);
                    }
                }
            } else {
                // extend the buf with some new data
                self.buf.resize(frame.stream_offset, 0);
                self.buf.extend_from_slice(frame.data);
                self.unfilled_offset = self.buf.len();
            }

            if self.expected_length == 0 {
                if let Some(header) = HandshakeHeader::try_parse(&self.buf[..self.unfilled_offset])
                {
                    if header.msg_length > self.max_message_size {
                        // use the same size limit as TLS record
                        return Err(FrameParseError::MalformedFrame(
                            "too large message length for TLS handshake",
                        ));
                    }
                    self.expected_length = header.msg_length as usize + HandshakeHeader::SIZE;
                    self.header = Some(header);
                    self.buf.resize(self.expected_length, 0);
                }
            }

            Ok(())
        } else if self.expected_length == 0 {
            // drop all other frames if we don't know the real length
            Err(FrameParseError::OutOfOrderFrame(
                "handshake header missing when receiving from crypto frame",
            ))
        } else if frame.stream_offset > self.expected_length {
            // invalid frames
            Err(FrameParseError::MalformedFrame(
                "too big stream offset value in crypto frame",
            ))
        } else {
            let frame_stream_end = frame.stream_offset + frame.data.len();
            if frame_stream_end > self.expected_length {
                // invalid frames
                return Err(FrameParseError::MalformedFrame(
                    "more data than expected in crypto frame",
                ));
            }
            let dst = &mut self.buf[frame.stream_offset..frame_stream_end];
            unsafe {
                std::ptr::copy_nonoverlapping(
                    frame.data.as_ptr(),
                    dst.as_mut_ptr(),
                    frame.data.len(),
                );
            }
            self.fragment_set.insert(Fragment {
                offset: frame.stream_offset,
                length: frame.data.len(),
            });

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;

    #[test]
    fn crypto_frame() {
        let data = hex!(
            "0040f1010000ed0303ebf8fa56f129 39b9584a3896472ec40bb863cfd3e868
            04fe3a47f06a2b69484c000004130113 02010000c000000010000e00000b6578
            616d706c652e636f6dff01000100000a 00080006001d00170018001000070005
            04616c706e0005000501000000000033 00260024001d00209370b2c9caa47fba
            baf4559fedba753de171fa71f50f1ce1 5d43e994ec74d748002b000302030400
            0d0010000e0403050306030203080408 050806002d00020101001c0002400100
            3900320408ffffffffffffffff050480 00ffff07048000ffff08011001048000
            75300901100f088394c8f03e51570806 048000ffff"
        );
        let frame = CryptoFrame::parse(&data).unwrap();
        assert_eq!(frame.stream_offset, 0);
        assert_eq!(frame.data.len(), data.len() - 3);
    }

    #[test]
    fn client_hello_consume() {
        let data = hex!(
            "010000ed0303ebf8fa56f129 39b9584a3896472ec40bb863cfd3e868
            04fe3a47f06a2b69484c000004130113 02010000c000000010000e00000b6578
            616d706c652e636f6dff01000100000a 00080006001d00170018001000070005
            04616c706e0005000501000000000033 00260024001d00209370b2c9caa47fba
            baf4559fedba753de171fa71f50f1ce1 5d43e994ec74d748002b000302030400
            0d0010000e0403050306030203080408 050806002d00020101001c0002400100
            3900320408ffffffffffffffff050480 00ffff07048000ffff08011001048000
            75300901100f088394c8f03e51570806 048000ffff"
        );

        let mut consumer = HandshakeCoalescer::default();
        let frame_full = CryptoFrame {
            stream_offset: 0,
            data: &data,
            encoded_len: 0,
        };
        consumer.recv_crypto(&frame_full).unwrap();
        assert!(consumer.finished());
        assert_eq!(consumer.buf, data);

        let mut consumer = HandshakeCoalescer::default();
        let frame1 = CryptoFrame {
            stream_offset: 0,
            data: &data[..30],
            encoded_len: 0,
        };
        let frame2 = CryptoFrame {
            stream_offset: 30,
            data: &data[30..],
            encoded_len: 0,
        };
        consumer.recv_crypto(&frame1).unwrap();
        assert!(!consumer.finished());
        consumer.recv_crypto(&frame2).unwrap();
        assert!(consumer.finished());
        assert_eq!(consumer.buf, data);

        let mut consumer = HandshakeCoalescer::default();
        let frame1 = CryptoFrame {
            stream_offset: 0,
            data: &data[..2],
            encoded_len: 0,
        };
        let frame2 = CryptoFrame {
            stream_offset: 2,
            data: &data[2..30],
            encoded_len: 0,
        };
        let frame3 = CryptoFrame {
            stream_offset: 28,
            data: &data[28..],
            encoded_len: 0,
        };
        consumer.recv_crypto(&frame1).unwrap();
        assert!(!consumer.finished());
        consumer.recv_crypto(&frame2).unwrap();
        assert!(!consumer.finished());
        consumer.recv_crypto(&frame3).unwrap();
        assert!(consumer.finished());
        assert_eq!(consumer.buf, data);

        let mut consumer = HandshakeCoalescer::default();
        let frame1 = CryptoFrame {
            stream_offset: 0,
            data: &data[..4],
            encoded_len: 0,
        };
        let frame2 = CryptoFrame {
            stream_offset: 30,
            data: &data[30..],
            encoded_len: 0,
        };
        let frame3 = CryptoFrame {
            stream_offset: 4,
            data: &data[4..30],
            encoded_len: 0,
        };
        consumer.recv_crypto(&frame1).unwrap();
        assert!(!consumer.finished());
        consumer.recv_crypto(&frame2).unwrap();
        assert!(!consumer.finished());
        consumer.recv_crypto(&frame3).unwrap();
        assert!(consumer.finished());
        assert_eq!(consumer.buf, data);

        let mut consumer = HandshakeCoalescer::default();
        let frame1 = CryptoFrame {
            stream_offset: 0,
            data: &data[..2],
            encoded_len: 0,
        };
        let frame2 = CryptoFrame {
            stream_offset: 30,
            data: &data[30..],
            encoded_len: 0,
        };
        let frame3 = CryptoFrame {
            stream_offset: 2,
            data: &data[2..30],
            encoded_len: 0,
        };
        let frame4 = CryptoFrame {
            stream_offset: 30,
            data: &data[30..],
            encoded_len: 0,
        };
        consumer.recv_crypto(&frame1).unwrap();
        assert!(!consumer.finished());
        assert!(consumer.recv_crypto(&frame2).is_err());
        assert!(!consumer.finished());
        consumer.recv_crypto(&frame3).unwrap();
        assert!(!consumer.finished());
        consumer.recv_crypto(&frame4).unwrap();
        assert!(consumer.finished());
        assert_eq!(consumer.buf, data);

        let mut consumer = HandshakeCoalescer::default();
        let frame1 = CryptoFrame {
            stream_offset: 0,
            data: &data[..4],
            encoded_len: 0,
        };
        let frame2 = CryptoFrame {
            stream_offset: 30,
            data: &data[30..40],
            encoded_len: 0,
        };
        let frame3 = CryptoFrame {
            stream_offset: 40,
            data: &data[40..],
            encoded_len: 0,
        };
        let frame4 = CryptoFrame {
            stream_offset: 4,
            data: &data[4..30],
            encoded_len: 0,
        };
        consumer.recv_crypto(&frame1).unwrap();
        assert!(!consumer.finished());
        consumer.recv_crypto(&frame2).unwrap();
        assert!(!consumer.finished());
        consumer.recv_crypto(&frame3).unwrap();
        assert!(!consumer.finished());
        consumer.recv_crypto(&frame4).unwrap();
        assert!(consumer.finished());
        assert_eq!(consumer.buf, data);

        let mut consumer = HandshakeCoalescer::default();
        let frame1 = CryptoFrame {
            stream_offset: 0,
            data: &data[..30],
            encoded_len: 0,
        };
        let frame2 = CryptoFrame {
            stream_offset: usize::MAX,
            data: &data[30..],
            encoded_len: 0,
        };
        consumer.recv_crypto(&frame1).unwrap();
        assert!(!consumer.finished());
        assert!(consumer.recv_crypto(&frame2).is_err());
    }
}
