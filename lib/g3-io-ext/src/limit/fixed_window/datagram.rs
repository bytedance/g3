/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::FixedWindow;
use crate::limit::DatagramLimitAction;

#[derive(Default)]
pub struct LocalDatagramLimiter {
    window: FixedWindow,

    // direct conf entry
    max_packets: usize,
    max_bytes: usize,

    // runtime record entry
    time_slice_id: u64,
    cur_packets: usize,
    cur_bytes: usize,
}

impl LocalDatagramLimiter {
    pub fn new(shift_millis: u8, max_packets: usize, max_bytes: usize) -> Self {
        LocalDatagramLimiter {
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

    pub fn check_packet(&mut self, cur_millis: u64, buf_size: usize) -> DatagramLimitAction {
        let time_slice_id = self.window.slice_id(cur_millis);
        if self.time_slice_id != time_slice_id {
            self.cur_bytes = 0;
            self.cur_packets = 0;
            self.time_slice_id = time_slice_id;
        }

        // do packet limit first. The first packet will always pass.
        if self.max_packets > 0 && self.cur_packets >= self.max_packets {
            return DatagramLimitAction::DelayFor(self.window.delay(cur_millis));
        }

        // always allow the first packet to pass
        if self.max_bytes > 0 && self.cur_bytes > 0 && self.cur_bytes + buf_size >= self.max_bytes {
            return DatagramLimitAction::DelayFor(self.window.delay(cur_millis));
        }
        // the real advance size should be set via set_advance_size() method by caller

        DatagramLimitAction::Advance(1)
    }

    pub fn check_packets(
        &mut self,
        cur_millis: u64,
        total_size_v: &[usize],
    ) -> DatagramLimitAction {
        let time_slice_id = self.window.slice_id(cur_millis);
        if self.time_slice_id != time_slice_id {
            self.cur_bytes = 0;
            self.cur_packets = 0;
            self.time_slice_id = time_slice_id;
        }

        let mut pkt_count = total_size_v.len();
        // do packet limit first. The first packet will always pass.
        if self.max_packets > 0 {
            if self.cur_packets >= self.max_packets {
                return DatagramLimitAction::DelayFor(self.window.delay(cur_millis));
            } else {
                pkt_count = pkt_count.min(self.max_packets - self.cur_packets);
            }
        }

        if self.max_bytes > 0 {
            if self.cur_bytes >= self.max_bytes {
                return DatagramLimitAction::DelayFor(self.window.delay(cur_millis));
            }

            let allowed = self.max_bytes - self.cur_bytes;
            pkt_count = match total_size_v[..pkt_count].binary_search(&allowed) {
                Ok(found_index) => found_index + 1,
                Err(0) => {
                    if self.cur_bytes == 0 {
                        // always allow the first packet in the window
                        1
                    } else {
                        return DatagramLimitAction::DelayFor(self.window.delay(cur_millis));
                    }
                }
                Err(insert_index) => insert_index,
            };
        }
        // the real advance size should be set via set_advance_size() method by caller

        DatagramLimitAction::Advance(pkt_count)
    }

    #[inline]
    pub fn set_advance(&mut self, packets: usize, size: usize) {
        self.cur_packets += packets;
        self.cur_bytes += size;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_packet_max_packets() {
        let mut limiter = LocalDatagramLimiter::new(10, 2, 0); // 2 packets max
        let cur_millis = 1000;

        // Packet 1: should pass
        assert!(matches!(
            limiter.check_packet(cur_millis, 100),
            DatagramLimitAction::Advance(1)
        ));
        limiter.set_advance(1, 100);

        // Packet 2: should pass
        assert!(matches!(
            limiter.check_packet(cur_millis, 100),
            DatagramLimitAction::Advance(1)
        ));
        limiter.set_advance(1, 100);

        // Packet 3: max_packets reached (2), should be delayed
        assert!(matches!(
            limiter.check_packet(cur_millis, 100),
            DatagramLimitAction::DelayFor(_)
        ));
    }

    #[test]
    fn test_check_packets_consistency() {
        let mut limiter = LocalDatagramLimiter::new(10, 2, 0); // 2 packets max
        let cur_millis = 1000;

        // Check 2 packets together
        assert!(matches!(
            limiter.check_packets(cur_millis, &[100, 200]),
            DatagramLimitAction::Advance(2)
        ));
        limiter.set_advance(2, 300);

        // Third packet checked alone should delay
        assert!(matches!(
            limiter.check_packet(cur_millis, 100),
            DatagramLimitAction::DelayFor(_)
        ));

        // Third packet checked in batch should delay
        assert!(matches!(
            limiter.check_packets(cur_millis, &[100]),
            DatagramLimitAction::DelayFor(_)
        ));
    }
}
