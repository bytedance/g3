/*
 * SPDX-License-Identifier: Apache-2.0
 * SPDX-FileCopyrightText: 2023-2025 ByteDance and/or its affiliates.
 * SPDX-FileCopyrightText: 2026 VEY-OSS Developers.
 */

use std::cell::UnsafeCell;
use std::os::fd::RawFd;
use std::pin::Pin;
#[cfg(ossl300)]
use std::sync::Arc;
use std::task::{Context, Poll, ready};
use std::time::Duration;
use std::{io, mem, ptr};

use anyhow::anyhow;
#[cfg(ossl300)]
use atomic_waker::AtomicWaker;
use libc::{c_int, c_void};
use openssl::error::ErrorStack;
use openssl::foreign_types::ForeignType;
use thiserror::Error;
use tokio::time::Sleep;

use super::AsyncWaitCtx;
use crate::ffi;

pub trait SyncOperation {
    type Output;

    fn run(&mut self) -> anyhow::Result<Self::Output>;
}

pub trait AsyncOperation: SyncOperation {
    fn track_raw_fd(&mut self, fd: RawFd) -> io::Result<()>;
    fn untrack_raw_fd(&mut self, fd: RawFd);
    fn poll_ready_fds(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>>;
}

#[derive(Debug, Error)]
pub enum OpensslAsyncTaskError {
    #[error("openssl error: {0}")]
    Openssl(#[from] ErrorStack),
    #[error("runtime error: {0:?}")]
    Runtime(#[from] io::Error),
    #[error("operation error: {0:?}")]
    Operation(anyhow::Error),
    #[error("unexpected error: {0}")]
    Unexpected(String),
}

/// Result of polling an [`OpensslAsyncTask`].
pub enum OpensslAsyncOutput<T: AsyncOperation> {
    /// Operation finished before the timeout (success or operation/openssl error).
    Finished(Result<T::Output, OpensslAsyncTaskError>),
    /// Timeout fired. Await [`cleanup`](Self::TimedOut::cleanup) on the same
    /// runtime thread to return any in-flight `ASYNC_JOB` to the pool before
    /// dropping it. `cleanup` is `None` when no job was in flight.
    TimedOut {
        cleanup: Option<OpensslAsyncCleanup<T>>,
    },
}

struct Action<T: AsyncOperation> {
    operation: T,
    result: anyhow::Result<T::Output>,
}

struct TaskState<T: AsyncOperation> {
    job: *mut ffi::ASYNC_JOB,
    wait_ctx: AsyncWaitCtx, // should be dropped before atomic_waker
    #[cfg(ossl300)]
    atomic_waker: Arc<AtomicWaker>,
    action: Box<UnsafeCell<Action<T>>>,
}

pub struct OpensslAsyncTask<T: AsyncOperation> {
    state: Option<TaskState<T>>,
    sleep_future: Pin<Box<Sleep>>,
}

/// Drains an in-flight OpenSSL `ASYNC_JOB` after a timeout.
///
/// Must be polled on the same current-thread runtime that started the job.
pub struct OpensslAsyncCleanup<T: AsyncOperation> {
    state: TaskState<T>,
}

/// NOTE: OpensslAsyncTask in fact is not Send,
/// make sure you call it in a single threaded async runtime
unsafe impl<T: AsyncOperation + Send> Send for OpensslAsyncTask<T> {}
unsafe impl<T: AsyncOperation + Send> Send for OpensslAsyncCleanup<T> {}

impl<T: AsyncOperation> TaskState<T> {
    #[cfg(not(ossl300))]
    fn new(operation: T) -> Result<Self, ErrorStack> {
        let wait_ctx = AsyncWaitCtx::new()?;
        Ok(TaskState {
            job: ptr::null_mut(),
            wait_ctx,
            action: Box::new(UnsafeCell::new(Action {
                operation,
                result: Err(anyhow!("not run yet")),
            })),
        })
    }

    #[cfg(ossl300)]
    fn new(operation: T) -> Result<Self, ErrorStack> {
        let atomic_waker = Arc::new(AtomicWaker::new());
        let wait_ctx = AsyncWaitCtx::new()?;
        wait_ctx.set_callback(&atomic_waker)?;
        Ok(TaskState {
            job: ptr::null_mut(),
            wait_ctx,
            atomic_waker,
            action: Box::new(UnsafeCell::new(Action {
                operation,
                result: Err(anyhow!("not run yet")),
            })),
        })
    }

    fn start_job(&mut self, ret: &mut c_int) -> c_int {
        let mut param = self.action.get();
        unsafe {
            ffi::ASYNC_start_job(
                &mut self.job,
                self.wait_ctx.as_ptr(),
                ret,
                Some(start_job::<T>),
                ptr::from_mut(&mut param).cast(),
                size_of::<*mut Action<T>>(),
            )
        }
    }

    #[cfg(not(ossl300))]
    fn poll_job(&mut self, cx: &mut Context<'_>) -> Poll<Result<T::Output, OpensslAsyncTaskError>> {
        let mut ret: c_int = 0;

        loop {
            match self.start_job(&mut ret) {
                ffi::ASYNC_ERR => return Poll::Ready(Err(ErrorStack::get().into())),
                ffi::ASYNC_NO_JOBS => {
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
                ffi::ASYNC_PAUSE => {
                    let action = unsafe { &mut *self.action.get() };
                    let (add, del) = self.wait_ctx.get_changed_fds()?;
                    for fd in add {
                        action.operation.track_raw_fd(fd)?;
                    }
                    for fd in del {
                        action.operation.untrack_raw_fd(fd);
                    }
                    ready!(action.operation.poll_ready_fds(cx))?;
                }
                ffi::ASYNC_FINISH => {
                    let action = unsafe { &mut *self.action.get() };
                    let r = mem::replace(&mut action.result, Err(anyhow!("")));
                    return Poll::Ready(r.map_err(OpensslAsyncTaskError::Operation));
                }
                r => {
                    return Poll::Ready(Err(OpensslAsyncTaskError::Unexpected(format!(
                        "ASYNC_start_job returned {r}"
                    ))));
                }
            }
        }
    }

    #[cfg(ossl300)]
    fn poll_job(&mut self, cx: &mut Context<'_>) -> Poll<Result<T::Output, OpensslAsyncTaskError>> {
        let mut ret: c_int = 0;

        self.atomic_waker.register(cx.waker());

        loop {
            match self.start_job(&mut ret) {
                ffi::ASYNC_ERR => return Poll::Ready(Err(ErrorStack::get().into())),
                ffi::ASYNC_NO_JOBS => {
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
                ffi::ASYNC_PAUSE => {
                    let action = unsafe { &mut *self.action.get() };
                    match self.wait_ctx.get_callback_status() {
                        ffi::ASYNC_STATUS_UNSUPPORTED => {
                            let (add, del) = self.wait_ctx.get_changed_fds()?;
                            for fd in add {
                                action.operation.track_raw_fd(fd)?;
                            }
                            for fd in del {
                                action.operation.untrack_raw_fd(fd);
                            }
                            ready!(action.operation.poll_ready_fds(cx))?;
                        }
                        ffi::ASYNC_STATUS_ERR => return Poll::Ready(Err(ErrorStack::get().into())),
                        ffi::ASYNC_STATUS_OK => return Poll::Pending,
                        ffi::ASYNC_STATUS_EAGAIN => {
                            cx.waker().wake_by_ref();
                            return Poll::Pending;
                        }
                        r => {
                            return Poll::Ready(Err(OpensslAsyncTaskError::Unexpected(format!(
                                "ASYNC_WAIT_CTX_get_status returned {r}"
                            ))));
                        }
                    }
                }
                ffi::ASYNC_FINISH => {
                    let action = unsafe { &mut *self.action.get() };
                    let r = mem::replace(&mut action.result, Err(anyhow!("")));
                    return Poll::Ready(r.map_err(OpensslAsyncTaskError::Operation));
                }
                r => {
                    return Poll::Ready(Err(OpensslAsyncTaskError::Unexpected(format!(
                        "ASYNC_start_job returned {r}"
                    ))));
                }
            }
        }
    }

    /// Resume until `ASYNC_FINISH` without starting a new job when `job` is null.
    fn poll_drain(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), OpensslAsyncTaskError>> {
        if self.job.is_null() {
            return Poll::Ready(Ok(()));
        }

        match ready!(self.poll_job(cx)) {
            Ok(_) | Err(OpensslAsyncTaskError::Operation(_)) => Poll::Ready(Ok(())),
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

impl<T: AsyncOperation> OpensslAsyncTask<T> {
    pub(crate) fn new(operation: T, timeout: Duration) -> Result<Self, ErrorStack> {
        Ok(OpensslAsyncTask {
            state: Some(TaskState::new(operation)?),
            sleep_future: Box::pin(tokio::time::sleep(timeout)),
        })
    }

    fn poll_run(&mut self, cx: &mut Context<'_>) -> Poll<OpensslAsyncOutput<T>> {
        match self.sleep_future.as_mut().poll(cx) {
            Poll::Pending => {}
            Poll::Ready(()) => {
                let cleanup = match self.state.take() {
                    Some(state) if !state.job.is_null() => Some(OpensslAsyncCleanup { state }),
                    _ => None,
                };
                return Poll::Ready(OpensslAsyncOutput::TimedOut { cleanup });
            }
        }

        let Some(state) = self.state.as_mut() else {
            return Poll::Ready(OpensslAsyncOutput::TimedOut { cleanup: None });
        };

        match ready!(state.poll_job(cx)) {
            Ok(v) => {
                self.state = None;
                Poll::Ready(OpensslAsyncOutput::Finished(Ok(v)))
            }
            Err(e) => {
                self.state = None;
                Poll::Ready(OpensslAsyncOutput::Finished(Err(e)))
            }
        }
    }
}

impl<T: AsyncOperation> Drop for OpensslAsyncTask<T> {
    fn drop(&mut self) {
        // Cancellation is not supported once a job is in flight. On timeout the
        // job is moved into [`OpensslAsyncCleanup`]; await that instead of dropping
        // this task early. See:
        //   - https://github.com/intel/QAT_Engine/issues/292
        //   - https://github.com/openssl/openssl/discussions/23158
        if let Some(state) = self.state.as_ref() {
            debug_assert!(
                state.job.is_null(),
                "OpensslAsyncTask dropped with an in-flight ASYNC_JOB"
            );
        }
    }
}

impl<T: AsyncOperation> Drop for OpensslAsyncCleanup<T> {
    fn drop(&mut self) {
        debug_assert!(
            self.state.job.is_null(),
            "OpensslAsyncCleanup dropped with an in-flight ASYNC_JOB"
        );
    }
}

extern "C" fn start_job<T: AsyncOperation>(arg: *mut c_void) -> c_int {
    let action_p = unsafe { arg.cast::<*mut Action<T>>().as_ref().unwrap() };
    let action = unsafe { action_p.as_mut().unwrap() };
    action.result = action.operation.run();
    0
}

impl<T> Future for OpensslAsyncTask<T>
where
    T: AsyncOperation + Unpin,
    T::Output: Unpin,
{
    type Output = OpensslAsyncOutput<T>;

    /// # Cancellation
    ///
    /// Not supported while an `ASYNC_JOB` is in flight. Use the built-in timeout:
    /// after it fires this future yields [`OpensslAsyncOutput::TimedOut`] with an
    /// optional [`OpensslAsyncCleanup`] that must be awaited on the same thread.
    /// Do not wrap this future in `tokio::time::timeout`.
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().poll_run(cx)
    }
}

impl<T> Future for OpensslAsyncCleanup<T>
where
    T: AsyncOperation + Unpin,
{
    type Output = Result<(), OpensslAsyncTaskError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().state.poll_drain(cx)
    }
}
