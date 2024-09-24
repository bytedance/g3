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

use std::cell::UnsafeCell;

pub struct GlobalInit<T> {
    inner: UnsafeCell<T>,
}

unsafe impl<T: Sync> Sync for GlobalInit<T> {}

impl<T> GlobalInit<T> {
    pub const fn new(value: T) -> Self {
        GlobalInit {
            inner: UnsafeCell::new(value),
        }
    }

    pub fn with_mut<F, R>(&self, handle: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        handle(unsafe { &mut *self.inner.get() })
    }
}

impl<T> AsRef<T> for GlobalInit<T> {
    fn as_ref(&self) -> &T {
        unsafe { &*self.inner.get() }
    }
}
