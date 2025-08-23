/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use thiserror::Error;

mod client_hello;
pub use client_hello::{ClientHello, ClientHelloParseError};

#[allow(dead_code)]
#[repr(u8)]
#[non_exhaustive]
pub enum HandshakeType {
    HelloRequestReserved = 0,
    ClientHello = 1,
    ServerHello = 2,
    HelloVerifyRequestReserved = 3,
    // There are more that we don't need, see
    // https://datatracker.ietf.org/doc/html/rfc8446#appendix-B.3
}

pub struct HandshakeHeader {
    pub msg_type: u8,
    pub msg_length: u32,
}

impl HandshakeHeader {
    pub const SIZE: usize = 4;

    /// Try to parse the data as a HandshakeHeader
    pub fn try_parse(data: &[u8]) -> Option<Self> {
        if data.len() < Self::SIZE {
            return None;
        }

        Some(HandshakeHeader {
            msg_type: data[0],
            msg_length: u32::from_be_bytes([0, data[1], data[2], data[3]]),
        })
    }

    /// Get the capacity needed to hold the encoded Handshake message
    pub fn encoded_cap(&self) -> usize {
        Self::SIZE + self.msg_length as usize
    }
}

pub struct HandshakeMessage<'a> {
    header: HandshakeHeader,
    msg_data: &'a [u8],
}

impl<'a> HandshakeMessage<'a> {
    /// Try to parse a full Handshake message from a fragment of data
    pub fn try_parse_fragment(data: &'a [u8]) -> Option<Self> {
        let header = HandshakeHeader::try_parse(data)?;
        let cap = header.encoded_cap();
        if cap <= data.len() {
            Some(HandshakeMessage {
                header,
                msg_data: &data[..cap],
            })
        } else {
            None
        }
    }

    /// Get the total length of this message on the wire
    pub fn encoded_len(&self) -> usize {
        self.msg_data.len()
    }

    /// Parse this message as a ClientHello message
    pub fn parse_client_hello(self) -> Result<ClientHello<'a>, ClientHelloParseError> {
        ClientHello::parse_fragment(self.header, self.msg_data)
    }
}

#[derive(Debug, Error)]
pub enum HandshakeCoalesceError {
    #[error("invalid content type {0}")]
    InvalidContentType(u8),
    #[error("too large message size {0}")]
    TooLargeMessageSize(u32),
}

pub struct HandshakeCoalescer {
    max_message_size: u32,
    header: Option<HandshakeHeader>,
    buf: Vec<u8>,
}

impl Default for HandshakeCoalescer {
    fn default() -> Self {
        HandshakeCoalescer::new(1 << 14)
    }
}

impl HandshakeCoalescer {
    pub fn new(max_message_size: u32) -> Self {
        HandshakeCoalescer {
            max_message_size,
            header: None,
            buf: Vec::new(),
        }
    }

    fn add_first_fragment(&mut self, data: &[u8]) -> Result<usize, HandshakeCoalesceError> {
        match HandshakeHeader::try_parse(data) {
            Some(hdr) => {
                if hdr.msg_length > self.max_message_size {
                    return Err(HandshakeCoalesceError::TooLargeMessageSize(hdr.msg_length));
                }
                let cap = hdr.encoded_cap();
                self.header = Some(hdr);
                if cap >= data.len() {
                    self.buf.reserve(cap);
                    self.buf.extend_from_slice(data);
                    Ok(data.len())
                } else {
                    self.buf.extend_from_slice(&data[..cap]);
                    Ok(cap)
                }
            }
            None => {
                self.buf.extend_from_slice(data);
                Ok(data.len())
            }
        }
    }

    pub fn coalesce_fragment(&mut self, data: &[u8]) -> Result<usize, HandshakeCoalesceError> {
        if self.buf.is_empty() {
            return self.add_first_fragment(data);
        }

        match &self.header {
            Some(hdr) => {
                let needed = hdr.encoded_cap() - self.buf.len();
                if needed >= data.len() {
                    self.buf.extend_from_slice(data);
                    Ok(data.len())
                } else {
                    self.buf.extend_from_slice(&data[..needed]);
                    Ok(needed)
                }
            }
            None => {
                self.buf.extend_from_slice(data);
                match HandshakeHeader::try_parse(&self.buf) {
                    Some(hdr) => {
                        if hdr.msg_length > self.max_message_size {
                            return Err(HandshakeCoalesceError::TooLargeMessageSize(
                                hdr.msg_length,
                            ));
                        }
                        let cap = hdr.encoded_cap();
                        self.header = Some(hdr);
                        if cap > self.buf.len() {
                            self.buf.reserve(cap - self.buf.len());
                            Ok(data.len())
                        } else {
                            self.buf.resize(cap, 0);
                            let consumed = data.len() - (self.buf.len() - cap);
                            Ok(consumed)
                        }
                    }
                    None => Ok(data.len()),
                }
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Parse this message as a ClientHello message
    pub fn parse_client_hello(&self) -> Result<Option<ClientHello<'_>>, ClientHelloParseError> {
        let Some(hdr) = &self.header else {
            return Ok(None);
        };
        if hdr.msg_type != HandshakeType::ClientHello as u8 {
            return Err(ClientHelloParseError::InvalidMessageType(hdr.msg_type));
        }
        if hdr.encoded_cap() == self.buf.len() {
            let ch = ClientHello::parse_msg_data(&self.buf[HandshakeHeader::SIZE..])?;
            Ok(Some(ch))
        } else {
            Ok(None)
        }
    }
}
