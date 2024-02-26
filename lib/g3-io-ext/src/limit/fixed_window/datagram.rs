/*
 * Copyright 2023 ByteDance and/or its affiliates.
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

use std::io::{IoSlice, IoSliceMut};

use super::FixedWindow;
use crate::RecvMsgHdr;

pub trait HasPacketSize {
    fn packet_size(&self) -> usize;
}

impl<'a> HasPacketSize for IoSlice<'a> {
    fn packet_size(&self) -> usize {
        self.len()
    }
}

impl<'a> HasPacketSize for IoSliceMut<'a> {
    fn packet_size(&self) -> usize {
        self.len()
    }
}

impl<'a, const C: usize> HasPacketSize for RecvMsgHdr<'a, C> {
    fn packet_size(&self) -> usize {
        self.n_recv
    }
}

#[cfg(feature = "quic")]
impl HasPacketSize for quinn::udp::Transmit {
    fn packet_size(&self) -> usize {
        self.contents.len()
    }
}

pub enum DatagramLimitResult {
    Advance(usize),
    DelayFor(u64),
}

#[derive(Default)]
pub struct DatagramLimitInfo {
    window: FixedWindow,

    // direct conf entry
    max_packets: usize,
    max_bytes: usize,

    // runtime record entry
    time_slice_id: u64,
    cur_packets: usize,
    cur_bytes: usize,
}

impl DatagramLimitInfo {
    pub fn new(shift_millis: u8, max_packets: usize, max_bytes: usize) -> Self {
        DatagramLimitInfo {
            window: FixedWindow::new(shift_millis, None),
            max_packets,
            max_bytes,
            time_slice_id: 0,
            cur_packets: 0,
            cur_bytes: 0,
        }
    }

    pub fn reset(
        &mut self,
        shift_millis: u8,
        max_packets: usize,
        max_bytes: usize,
        cur_millis: u64,
    ) {
        self.window = FixedWindow::new(shift_millis, Some(cur_millis));
        self.max_packets = max_packets;
        self.max_bytes = max_bytes;
        self.time_slice_id = self.window.slice_id(cur_millis);
        self.cur_packets = 0;
        self.cur_bytes = 0;
    }

    #[inline]
    pub fn is_set(&self) -> bool {
        self.window.enabled()
    }

    pub fn check_packet(&mut self, cur_millis: u64, buf_size: usize) -> DatagramLimitResult {
        let time_slice_id = self.window.slice_id(cur_millis);
        if self.time_slice_id != time_slice_id {
            self.cur_bytes = 0;
            self.cur_packets = 0;
            self.time_slice_id = time_slice_id;
        }

        // do packet limit first. The first packet will always pass.
        if self.max_packets > 0 && self.cur_packets > self.max_packets {
            return DatagramLimitResult::DelayFor(self.window.delay(cur_millis));
        }

        // always allow the first packet to pass
        if self.max_bytes > 0 && self.cur_bytes > 0 && self.cur_bytes + buf_size >= self.max_bytes {
            return DatagramLimitResult::DelayFor(self.window.delay(cur_millis));
        }
        // the real advance size should be set via set_advance_size() method by caller

        DatagramLimitResult::Advance(1)
    }

    pub fn check_packets<P>(&mut self, cur_millis: u64, packets: &[P]) -> DatagramLimitResult
    where
        P: HasPacketSize,
    {
        let time_slice_id = self.window.slice_id(cur_millis);
        if self.time_slice_id != time_slice_id {
            self.cur_bytes = 0;
            self.cur_packets = 0;
            self.time_slice_id = time_slice_id;
        }

        let mut pkt_count = packets.len();
        // do packet limit first. The first packet will always pass.
        if self.max_packets > 0 {
            if self.cur_packets > self.max_packets {
                return DatagramLimitResult::DelayFor(self.window.delay(cur_millis));
            } else {
                pkt_count = pkt_count.min(self.max_packets - self.cur_packets);
            }
        }

        if self.max_bytes > 0 {
            let mut total_size = 0usize;
            for (i, p) in packets.iter().enumerate().take(pkt_count) {
                total_size += p.packet_size();
                if self.cur_bytes == 0 {
                    // always allow the first packet
                    continue;
                }

                if self.cur_bytes + total_size >= self.max_bytes {
                    return if i == 0 {
                        DatagramLimitResult::DelayFor(self.window.delay(cur_millis))
                    } else {
                        DatagramLimitResult::Advance(i)
                    };
                }
            }
        }
        // the real advance size should be set via set_advance_size() method by caller

        DatagramLimitResult::Advance(pkt_count)
    }

    #[inline]
    pub fn set_advance(&mut self, packets: usize, size: usize) {
        self.cur_packets += packets;
        self.cur_bytes += size;
    }
}
