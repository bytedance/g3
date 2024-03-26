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

use std::ops::{Deref, DerefMut};
use std::os::fd::{FromRawFd, IntoRawFd, RawFd};

pub struct RawFdGuard<T>
where
    T: FromRawFd + IntoRawFd,
{
    inner: Option<T>,
}

impl<T> RawFdGuard<T>
where
    T: FromRawFd + IntoRawFd,
{
    pub fn new(fd: RawFd) -> Self {
        Self {
            inner: unsafe { Some(T::from_raw_fd(fd)) },
        }
    }
}

impl<T> Drop for RawFdGuard<T>
where
    T: FromRawFd + IntoRawFd,
{
    fn drop(&mut self) {
        if let Some(resource) = self.inner.take() {
            let _ = resource.into_raw_fd();
        }
    }
}

impl<T> Deref for RawFdGuard<T>
where
    T: FromRawFd + IntoRawFd,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // the only way setting inner to None is drop
        self.inner.as_ref().unwrap()
    }
}

impl<T> DerefMut for RawFdGuard<T>
where
    T: FromRawFd + IntoRawFd,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        // the only way setting inner to None is drop
        self.inner.as_mut().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::RawFdGuard;
    use socket2::Socket;

    #[test]
    fn not_close_fd() {
        let fd = 0;
        {
            let _socket = RawFdGuard::<Socket>::new(fd);
            // unsafe { libc::close(fd) };
        }
        assert!(unsafe { libc::fcntl(fd, libc::F_GETFD) } != -1);
    }
}
