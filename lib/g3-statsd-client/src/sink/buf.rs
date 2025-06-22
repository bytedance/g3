/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::io::IoSlice;

pub(super) struct SinkBuf {
    max_size: usize,
    buf: Vec<u8>,
    msg_length_vec: Vec<usize>,
}

impl SinkBuf {
    pub(super) fn new(capacity: usize) -> Self {
        SinkBuf {
            max_size: capacity,
            buf: Vec::with_capacity(capacity),
            msg_length_vec: Vec::new(),
        }
    }

    pub(super) fn reset(&mut self) {
        self.buf.clear();
        self.msg_length_vec.clear();
    }

    pub(super) fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub(super) fn buf_full(&self) -> bool {
        self.buf.len() >= self.max_size
    }

    pub(super) fn receive<F>(&mut self, mut format: F)
    where
        F: FnMut(&mut Vec<u8>),
    {
        let start = self.buf.len();
        format(&mut self.buf);
        let end = self.buf.len();
        if end > start {
            self.msg_length_vec.push(end - start);
        }
    }

    pub(super) fn iter(&mut self, segment_size: usize) -> SinkBufIter<'_> {
        SinkBufIter {
            segment_size,
            buf: self,
            buf_offset: 0,
            msg_offset: 0,
        }
    }
}

pub(super) struct SinkBufIter<'a> {
    segment_size: usize,
    buf: &'a SinkBuf,
    buf_offset: usize,
    msg_offset: usize,
}

impl<'a> Iterator for SinkBufIter<'a> {
    type Item = IoSlice<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.msg_offset >= self.buf.msg_length_vec.len() {
            return None;
        }

        let left_length_list = &self.buf.msg_length_vec[self.msg_offset..];
        let left_data = &self.buf.buf[self.buf_offset..];

        let mut to_read = 0;
        for len in left_length_list {
            if to_read + *len > self.segment_size {
                break;
            }
            to_read += *len;
            self.msg_offset += 1;
        }
        if to_read == 0 {
            // the first msg is too large, let's write anyway
            to_read = left_length_list[0];
            self.msg_offset += 1;
        }
        self.buf_offset += to_read;
        Some(IoSlice::new(&left_data[..to_read]))
    }
}
