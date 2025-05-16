/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::os::fd::RawFd;
use std::task::{Context, Poll};

use openssl::error::ErrorStack;
use tokio::io::Interest;
use tokio::io::unix::AsyncFd;
use tokio::runtime::{Handle, RuntimeFlavor};

use super::{AsyncOperation, OpensslAsyncTask, SyncOperation};

pub struct TokioAsyncOperation<T> {
    sync_op: T,
    tracked_fds: Vec<AsyncFd<RawFd>>,
}

impl<T> TokioAsyncOperation<T>
where
    T: SyncOperation,
{
    /// Create a openssl async task in tokio single threaded runtime
    ///
    /// It will panic if called in multi-threaded runtime
    pub fn build_async_task(
        sync_op: T,
    ) -> Result<OpensslAsyncTask<TokioAsyncOperation<T>>, ErrorStack> {
        assert_eq!(
            Handle::current().runtime_flavor(),
            RuntimeFlavor::CurrentThread
        );

        let async_op = TokioAsyncOperation {
            sync_op,
            tracked_fds: Vec::with_capacity(1),
        };
        OpensslAsyncTask::new(async_op)
    }
}

impl<T: SyncOperation> SyncOperation for TokioAsyncOperation<T> {
    type Output = T::Output;

    fn run(&mut self) -> anyhow::Result<T::Output> {
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
