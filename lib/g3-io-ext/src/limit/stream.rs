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

use super::LocalStreamLimiter;

#[derive(Debug, Eq, PartialEq)]
pub enum StreamLimitAction {
    AdvanceBy(usize),
    DelayFor(u64),
}

#[derive(Default)]
pub struct StreamLimiter {
    is_set: bool,
    local: LocalStreamLimiter,
}

impl StreamLimiter {
    pub fn with_local(shift_millis: u8, max_bytes: usize) -> Self {
        let local = LocalStreamLimiter::new(shift_millis, max_bytes);
        let is_set = local.is_set();
        StreamLimiter { is_set, local }
    }

    pub fn reset_local(&mut self, shift_millis: u8, max_bytes: usize, cur_millis: u64) {
        self.local.reset(shift_millis, max_bytes, cur_millis);
        self.is_set |= self.local.is_set();
    }

    #[inline]
    pub fn is_set(&self) -> bool {
        self.is_set
    }

    pub fn check(&mut self, cur_millis: u64, to_advance: usize) -> StreamLimitAction {
        self.local.check(cur_millis, to_advance)
    }

    pub fn set_advance(&mut self, size: usize) {
        self.local.set_advance(size);
    }
}
