/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
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

    pub fn set(&self, value: T) {
        unsafe {
            let inner_mut = &mut *self.inner.get();
            *inner_mut = value;
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
