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

use super::FixedWindow;

#[derive(Debug, Eq, PartialEq)]
pub enum StreamLimitResult {
    AdvanceBy(usize),
    DelayFor(u64),
}

#[derive(Default)]
pub struct StreamLimitInfo {
    window: FixedWindow,

    // direct conf entry
    max_bytes: usize,

    // runtime record entry
    time_slice_id: u64,
    cur_bytes: usize,
}

impl StreamLimitInfo {
    pub fn new(shift_millis: u8, max_bytes: usize) -> Self {
        StreamLimitInfo {
            window: FixedWindow::new(shift_millis, None),
            max_bytes,
            time_slice_id: 0,
            cur_bytes: 0,
        }
    }

    pub fn reset(&mut self, shift_millis: u8, max_bytes: usize, cur_millis: u64) {
        self.window = FixedWindow::new(shift_millis, Some(cur_millis));
        self.max_bytes = max_bytes;
        self.time_slice_id = self.window.slice_id(cur_millis);
        self.cur_bytes = 0;
    }

    #[inline]
    pub fn is_set(&self) -> bool {
        self.window.enabled()
    }

    pub fn check(&mut self, cur_millis: u64, to_advance: usize) -> StreamLimitResult {
        let time_slice_id = self.window.slice_id(cur_millis);
        if self.time_slice_id != time_slice_id {
            self.cur_bytes = 0;
            self.time_slice_id = time_slice_id;
        }

        let max = self.max_bytes - self.cur_bytes;
        if max == 0 {
            StreamLimitResult::DelayFor(self.window.delay(cur_millis))
        } else {
            let min = to_advance.min(max);
            StreamLimitResult::AdvanceBy(min)
        }
    }

    #[inline]
    pub fn set_advance(&mut self, size: usize) {
        self.cur_bytes += size;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_routine() {
        let mut limit = StreamLimitInfo::new(10, 1000);
        // new time slice
        // try to send 500
        assert_eq!(limit.check(0, 500), StreamLimitResult::AdvanceBy(500));
        limit.set_advance(500);
        // try to send 500
        assert_eq!(limit.check(10, 520), StreamLimitResult::AdvanceBy(500));
        limit.set_advance(500);
        // try to send 20, which should be delayed
        assert_eq!(limit.check(20, 20), StreamLimitResult::DelayFor(1004));
        // delay end, new time slice
        // try to send 20
        assert_eq!(limit.check(1024, 20), StreamLimitResult::AdvanceBy(20));
        limit.set_advance(20);
        // try to send 100
        assert_eq!(limit.check(1050, 100), StreamLimitResult::AdvanceBy(100));
        // only 80 really sent, roll back 20
        limit.set_advance(80);
        // try to send 900
        assert_eq!(limit.check(1100, 1000), StreamLimitResult::AdvanceBy(900));
        limit.set_advance(900);
    }

    // TODO add reset test case
}
