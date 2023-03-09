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

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GaugeSemaphoreAcquireError {
    #[error("overflow")]
    Overflow,
    #[error("no permits")]
    NoPermits,
}

pub struct GaugeSemaphorePermit {
    count: usize,
    gauge: Arc<AtomicUsize>,
}

impl Drop for GaugeSemaphorePermit {
    fn drop(&mut self) {
        self.gauge.fetch_sub(self.count, Ordering::Release);
    }
}

pub struct GaugeSemaphore {
    permits: usize,
    gauge: Arc<AtomicUsize>,
}

impl GaugeSemaphore {
    pub fn new(permits: usize) -> Self {
        GaugeSemaphore {
            permits,
            gauge: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Create a new semaphore with old Gauge value.
    ///
    /// The new permits size may be less than the current gauge value.
    #[must_use]
    pub fn new_updated(&self, permits: usize) -> Self {
        GaugeSemaphore {
            permits,
            gauge: Arc::clone(&self.gauge),
        }
    }

    #[inline]
    pub fn try_acquire(&self) -> Result<GaugeSemaphorePermit, GaugeSemaphoreAcquireError> {
        self.try_acquire_n(1)
    }

    pub fn try_acquire_n(
        &self,
        num_permits: usize,
    ) -> Result<GaugeSemaphorePermit, GaugeSemaphoreAcquireError> {
        let mut curr = self.gauge.load(Ordering::Acquire);
        loop {
            // check for overflow
            let next = match curr.checked_add(num_permits) {
                Some(n) => n,
                None => return Err(GaugeSemaphoreAcquireError::Overflow),
            };

            if self.permits > 0 && next > self.permits {
                return Err(GaugeSemaphoreAcquireError::NoPermits);
            }

            match self
                .gauge
                .compare_exchange(curr, next, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => {
                    return Ok(GaugeSemaphorePermit {
                        count: num_permits,
                        gauge: Arc::clone(&self.gauge),
                    });
                }
                Err(actual) => curr = actual,
            }
        }
    }

    pub fn gauge(&self) -> usize {
        self.gauge.load(Ordering::Acquire)
    }

    pub fn permits(&self) -> usize {
        self.permits
    }

    /// Return the number of available permits.
    ///
    /// Return `None` for disabled semaphores.
    /// Return `Some(0)` for semaphores that is already overloaded.
    pub fn available_permits(&self) -> Option<usize> {
        if self.permits == 0 {
            None
        } else {
            Some(self.permits.saturating_sub(self.gauge()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let sem = GaugeSemaphore::new(10);
        let p1 = sem.try_acquire_n(9).unwrap();
        assert!(sem.try_acquire_n(2).is_err());

        drop(p1);
        let p2 = sem.try_acquire_n(2).unwrap();

        let sem2 = sem.new_updated(15);
        let _p3 = sem2.try_acquire_n(10).unwrap();
        assert!(sem2.try_acquire_n(4).is_err());

        drop(p2);
        let _p4 = sem2.try_acquire_n(4).unwrap();
        assert!(sem2.try_acquire_n(2).is_err());
        assert_eq!(sem2.available_permits(), Some(1));

        let sem3 = sem2.new_updated(0);
        let p5 = sem3.try_acquire_n(2).unwrap();
        assert_eq!(sem3.available_permits(), None);

        let sem4 = sem3.new_updated(15);
        assert_eq!(sem4.available_permits(), Some(0));

        drop(p5);
        assert_eq!(sem4.available_permits(), Some(1));
    }
}
