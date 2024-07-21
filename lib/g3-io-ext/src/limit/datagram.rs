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

use std::io::{IoSlice, IoSliceMut};

use super::LocalDatagramLimiter;
#[cfg(unix)]
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

#[cfg(unix)]
impl<'a, const C: usize> HasPacketSize for RecvMsgHdr<'a, C> {
    fn packet_size(&self) -> usize {
        self.n_recv
    }
}

#[cfg(feature = "quic")]
impl<'a> HasPacketSize for quinn::udp::Transmit<'a> {
    fn packet_size(&self) -> usize {
        self.contents.len()
    }
}

pub enum DatagramLimitAction {
    Advance(usize),
    DelayFor(u64),
}

#[derive(Default)]
pub struct DatagramLimiter {
    is_set: bool,
    local: LocalDatagramLimiter,
}

impl DatagramLimiter {
    pub fn with_local(shift_millis: u8, max_packets: usize, max_bytes: usize) -> Self {
        let local = LocalDatagramLimiter::new(shift_millis, max_packets, max_bytes);
        let is_set = local.is_set();
        DatagramLimiter { is_set, local }
    }

    pub fn reset_local(
        &mut self,
        shift_millis: u8,
        max_packets: usize,
        max_bytes: usize,
        cur_millis: u64,
    ) {
        self.local
            .reset(shift_millis, max_packets, max_bytes, cur_millis);
        self.is_set |= self.local.is_set();
    }

    #[inline]
    pub fn is_set(&self) -> bool {
        self.is_set
    }

    pub fn check_packet(&mut self, cur_millis: u64, buf_size: usize) -> DatagramLimitAction {
        self.local.check_packet(cur_millis, buf_size)
    }

    pub fn check_packets<P>(&mut self, cur_millis: u64, packets: &[P]) -> DatagramLimitAction
    where
        P: HasPacketSize,
    {
        self.local.check_packets(cur_millis, packets)
    }

    pub fn set_advance(&mut self, packets: usize, size: usize) {
        self.local.set_advance(packets, size);
    }
}
