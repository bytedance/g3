/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

pub trait TlvParse<'a> {
    const TAG_SIZE: usize;
    const LENGTH_SIZE: usize;
    type Tag;
    type Error;

    fn tag(buf: &[u8]) -> Self::Tag;
    fn length(buf: &[u8]) -> usize;
    fn no_enough_data() -> Self::Error;
    fn parse_value(&mut self, tag: Self::Tag, buf: &'a [u8]) -> Result<(), Self::Error>;

    fn parse_tlv(&mut self, v: &'a [u8]) -> Result<(), Self::Error> {
        let total_len = v.len();
        let mut offset = 0usize;

        loop {
            if offset + Self::TAG_SIZE + Self::LENGTH_SIZE > total_len {
                return Err(Self::no_enough_data());
            }

            let buf = &v[offset..];
            let tag = Self::tag(&buf[0..Self::TAG_SIZE]);
            let vl = Self::length(&buf[Self::TAG_SIZE..]);
            offset += Self::TAG_SIZE + Self::LENGTH_SIZE;
            if offset + vl > total_len {
                return Err(Self::no_enough_data());
            }

            let buf = &v[offset..offset + vl];
            self.parse_value(tag, buf)?;
            offset += vl;
            if offset == total_len {
                return Ok(());
            }
        }
    }
}

pub trait T1L2BVParse<'a> {
    type Error;

    fn no_enough_data() -> Self::Error;
    fn parse_value(&mut self, tag: u8, buf: &'a [u8]) -> Result<(), Self::Error>;
}

impl<'a, T> TlvParse<'a> for T
where
    T: T1L2BVParse<'a>,
{
    const TAG_SIZE: usize = 1;
    const LENGTH_SIZE: usize = 2;
    type Tag = u8;
    type Error = T::Error;

    fn tag(buf: &[u8]) -> Self::Tag {
        buf[0]
    }

    fn length(buf: &[u8]) -> usize {
        u16::from_be_bytes([buf[0], buf[1]]) as usize
    }

    fn no_enough_data() -> Self::Error {
        T::no_enough_data()
    }

    fn parse_value(&mut self, tag: Self::Tag, buf: &'a [u8]) -> Result<(), Self::Error> {
        self.parse_value(tag, buf)
    }
}
