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

use std::sync::{Arc, Mutex};

use super::FixedWindow;

#[derive(Clone)]
struct InnerCountLimitInfo {
    window: FixedWindow,

    // direct conf entry
    max_count: usize,

    // runtime record entry
    time_slice_id: u64,
    cur_count: usize,
}

impl InnerCountLimitInfo {
    fn new(shift_millis: u8, max_count: usize) -> Self {
        InnerCountLimitInfo {
            window: FixedWindow::new(shift_millis, None),
            max_count,
            time_slice_id: 0,
            cur_count: max_count,
        }
    }

    fn reset(&mut self, shift_millis: u8, max_count: usize, cur_millis: u64) {
        self.window = FixedWindow::new(shift_millis, Some(cur_millis));
        self.max_count = max_count;
        self.time_slice_id = self.window.slice_id(cur_millis);
        self.cur_count = 0;
    }

    fn check(&mut self, cur_millis: u64) -> Result<(), u64> {
        let time_slice_id = self.window.slice_id(cur_millis);
        if self.time_slice_id != time_slice_id {
            self.cur_count = self.max_count;
            self.time_slice_id = time_slice_id;
        }

        if self.cur_count > 0 {
            self.cur_count -= 1;
            Ok(())
        } else {
            Err(self.window.delay(cur_millis))
        }
    }
}

#[derive(Clone)]
pub struct ThreadedCountLimitInfo(Arc<Mutex<InnerCountLimitInfo>>);

impl ThreadedCountLimitInfo {
    pub fn new(shift_millis: u8, max_count: usize) -> Self {
        ThreadedCountLimitInfo(Arc::new(Mutex::new(InnerCountLimitInfo::new(
            shift_millis,
            max_count,
        ))))
    }

    #[must_use]
    pub fn new_updated(&self, shift_millis: u8, max_count: usize, cur_millis: u64) -> Self {
        let inner = self.0.lock().unwrap();
        let mut inner = (*inner).clone();
        inner.reset(shift_millis, max_count, cur_millis);
        ThreadedCountLimitInfo(Arc::new(Mutex::new(inner)))
    }

    pub fn check(&self, cur_millis: u64) -> Result<(), u64> {
        let mut inner = self.0.lock().unwrap();
        inner.check(cur_millis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overflow() {
        let mut limit_info = InnerCountLimitInfo::new(10, 2);
        assert!(limit_info.check(1).is_ok());
        assert!(limit_info.check(2).is_ok());
        assert!(limit_info.check(3).is_err());
        assert!(limit_info.check(4).is_err());
        assert!(limit_info.check(1025).is_ok());
    }
}
