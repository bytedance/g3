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

use std::io;
use std::os::fd::RawFd;
use std::task::{Context, Poll};

use tokio::io::unix::AsyncFd;
use tokio::io::Interest;

use super::{AsyncOperation, SyncOperation};

pub struct TokioAsyncOperation<T> {
    sync_op: T,
    tracked_fds: Vec<AsyncFd<RawFd>>,
}

impl<T> TokioAsyncOperation<T> {
    pub fn new(sync_op: T) -> Self {
        TokioAsyncOperation {
            sync_op,
            tracked_fds: Vec::with_capacity(1),
        }
    }

    pub fn into_sync_op(self) -> T {
        self.sync_op
    }
}

impl<T: SyncOperation> SyncOperation for TokioAsyncOperation<T> {
    fn run(&mut self) -> anyhow::Result<()> {
        self.sync_op.run()
    }
}

impl<T: SyncOperation> AsyncOperation for TokioAsyncOperation<T> {
    fn track_raw_fd(&mut self, fd: RawFd) -> io::Result<()> {
        let async_fd = AsyncFd::with_interest(fd, Interest::READABLE)?;
        self.tracked_fds.push(async_fd);
        Ok(())
    }

    fn untrack_raw_fd(&mut self, fd: RawFd) {
        self.tracked_fds.retain(|v| fd.ne(v.get_ref()));
    }

    fn poll_ready_fds(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        for fd in &self.tracked_fds {
            match fd.poll_read_ready(cx) {
                Poll::Pending => {}
                Poll::Ready(Ok(_)) => return Poll::Ready(Ok(())),
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            }
        }
        Poll::Pending
    }
}
