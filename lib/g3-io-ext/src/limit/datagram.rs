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
use std::sync::Arc;

use super::LocalDatagramLimiter;
#[cfg(unix)]
use crate::{RecvMsgHdr, SendMsgHdr};

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

#[cfg(unix)]
impl<'a, const C: usize> HasPacketSize for SendMsgHdr<'a, C> {
    fn packet_size(&self) -> usize {
        self.n_send
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

pub trait GlobalDatagramLimit {
    fn check_packet(&self, buf_size: usize) -> DatagramLimitAction;
    fn check_packets(&self, packets: usize, buf_size: usize) -> DatagramLimitAction;
    fn release_size(&self, size: usize);
    fn release_packets(&self, packets: usize);
}

struct GlobalLimiter {
    inner: Arc<dyn GlobalDatagramLimit + Send + Sync>,
    checked_packets: Option<usize>,
    checked_size: Option<usize>,
}

impl GlobalLimiter {
    fn new<T>(inner: Arc<T>) -> Self
    where
        T: GlobalDatagramLimit + Send + Sync + 'static,
    {
        GlobalLimiter {
            inner,
            checked_packets: None,
            checked_size: None,
        }
    }
}

#[derive(Default)]
pub struct DatagramLimiter {
    is_set: bool,
    local: LocalDatagramLimiter,
    global: Vec<GlobalLimiter>,
}

impl DatagramLimiter {
    pub fn with_local(shift_millis: u8, max_packets: usize, max_bytes: usize) -> Self {
        let local = LocalDatagramLimiter::new(shift_millis, max_packets, max_bytes);
        let is_set = local.is_set();
        DatagramLimiter {
            is_set,
            local,
            global: Vec::new(),
        }
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
        if self.global.is_empty() {
            self.is_set = self.local.is_set();
        }
    }

    pub fn add_global<T>(&mut self, limiter: Arc<T>)
    where
        T: GlobalDatagramLimit + Send + Sync + 'static,
    {
        self.global.push(GlobalLimiter::new(limiter));
        self.is_set = true;
    }

    #[inline]
    pub fn is_set(&self) -> bool {
        self.is_set
    }

    pub fn check_packet(&mut self, cur_millis: u64, buf_size: usize) -> DatagramLimitAction {
        match self.local.check_packet(cur_millis, buf_size) {
            DatagramLimitAction::Advance(_) => {}
            DatagramLimitAction::DelayFor(n) => return DatagramLimitAction::DelayFor(n),
        };

        for limiter in &mut self.global {
            match limiter.inner.check_packet(buf_size) {
                DatagramLimitAction::Advance(_) => {
                    limiter.checked_packets = Some(1);
                    limiter.checked_size = Some(buf_size);
                }
                DatagramLimitAction::DelayFor(n) => {
                    self.release_global();
                    return DatagramLimitAction::DelayFor(n);
                }
            }
        }

        DatagramLimitAction::Advance(1)
    }

    pub fn check_packets<P>(&mut self, cur_millis: u64, packets: &[P]) -> DatagramLimitAction
    where
        P: HasPacketSize,
    {
        let target_packets = packets.len();
        let mut to_advance = match self.local.check_packets(cur_millis, packets) {
            DatagramLimitAction::Advance(n) => n,
            DatagramLimitAction::DelayFor(n) => return DatagramLimitAction::DelayFor(n),
        };
        if self.global.is_empty() {
            return DatagramLimitAction::Advance(to_advance);
        }

        let mut buf_size = packets
            .iter()
            .map(|v| v.packet_size())
            .take(to_advance)
            .sum();
        for limiter in &mut self.global {
            match limiter.inner.check_packets(to_advance, buf_size) {
                DatagramLimitAction::Advance(n) => {
                    if n != to_advance {
                        to_advance = n;
                        buf_size = packets
                            .iter()
                            .map(|v| v.packet_size())
                            .take(to_advance)
                            .sum();
                    }
                    limiter.checked_packets = Some(n);
                }
                DatagramLimitAction::DelayFor(n) => {
                    self.release_global();
                    return DatagramLimitAction::DelayFor(n);
                }
            }
        }

        if target_packets > to_advance {
            for limiter in &mut self.global {
                let checked = limiter.checked_packets.take().unwrap();
                if checked > to_advance {
                    limiter.inner.release_packets(checked - to_advance);
                }
                limiter.checked_packets = Some(to_advance);
                limiter.checked_size = Some(buf_size);
            }
        }
        DatagramLimitAction::Advance(to_advance)
    }

    pub fn release_global(&mut self) {
        for limiter in &mut self.global {
            let Some(packets) = limiter.checked_packets.take() else {
                break;
            };
            limiter.inner.release_packets(packets);
            if let Some(size) = limiter.checked_size.take() {
                limiter.inner.release_size(size);
            }
        }
    }

    pub fn set_advance(&mut self, packets: usize, size: usize) {
        self.local.set_advance(packets, size);

        for limiter in &mut self.global {
            let Some(checked) = limiter.checked_packets.take() else {
                break;
            };

            if checked > packets {
                limiter.inner.release_packets(checked - packets);
            }

            if let Some(checked) = limiter.checked_size.take() {
                if checked > size {
                    limiter.inner.release_size(checked - size);
                }
            }
        }
    }
}
