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

use rand::Rng;

use g3_types::net::RATE_LIMIT_SHIFT_MILLIS_MAX;

mod count;
pub use count::ThreadedCountLimitInfo;

mod datagram;
mod stream;

pub use datagram::{DatagramLimitInfo, DatagramLimitResult};
pub use stream::{StreamLimitInfo, StreamLimitResult};

#[derive(Clone, Copy)]
struct FixedWindow {
    max_delay_millis: u64,
    time_value_mask: u64,
    time_slice_mask: u64,
    slice_id_offset: u64,
}

impl Default for FixedWindow {
    fn default() -> Self {
        FixedWindow {
            max_delay_millis: 1,
            time_value_mask: 0,
            time_slice_mask: u64::MAX,
            slice_id_offset: 0,
        }
    }
}

impl FixedWindow {
    fn new(shift_millis: u8, cur_millis: Option<u64>) -> Self {
        let mut shift = shift_millis;
        if shift > RATE_LIMIT_SHIFT_MILLIS_MAX {
            shift = RATE_LIMIT_SHIFT_MILLIS_MAX;
        }
        let max_delay_millis = 1_u64 << shift;
        let time_value_mask = max_delay_millis - 1;
        let time_slice_mask = u64::MAX ^ time_value_mask;

        let slice_id_offset = if let Some(cur_millis) = cur_millis {
            cur_millis - (cur_millis & time_slice_mask)
        } else {
            let mut rng = rand::thread_rng();
            rng.gen_range(0..=max_delay_millis)
        };

        FixedWindow {
            max_delay_millis,
            time_value_mask,
            time_slice_mask,
            slice_id_offset,
        }
    }

    fn enabled(&self) -> bool {
        self.max_delay_millis > 1 // which means shift != 0
    }

    fn slice_id(&self, cur_millis: u64) -> u64 {
        (self.time_slice_mask & cur_millis) + self.slice_id_offset
    }

    fn delay(&self, cur_millis: u64) -> u64 {
        self.max_delay_millis - (self.time_value_mask & cur_millis)
    }
}
