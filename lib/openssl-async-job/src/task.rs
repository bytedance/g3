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

use std::cell::UnsafeCell;
use std::future::Future;
use std::io;
use std::os::fd::RawFd;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{ready, Context, Poll};
use std::{mem, ptr};

use anyhow::anyhow;
use atomic_waker::AtomicWaker;
use libc::{c_int, c_void};
use openssl::error::ErrorStack;
use openssl::foreign_types::ForeignType;
use thiserror::Error;

use super::{ffi, AsyncWaitCtx};

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

struct Action<T: AsyncOperation> {
    operation: T,
    result: anyhow::Result<T::Output>,
}

pub struct OpensslAsyncTask<T: AsyncOperation> {
    job: *mut ffi::ASYNC_JOB,
    wait_ctx: AsyncWaitCtx, // should be dropped before atomic_waker
    atomic_waker: Arc<AtomicWaker>,
    action: Box<UnsafeCell<Action<T>>>,
}

/// NOTE: OpensslAsyncTask in fact is not Send,
/// make sure you call it in a single threaded async runtime
unsafe impl<T: AsyncOperation + Send> Send for OpensslAsyncTask<T> {}

impl<T: AsyncOperation> OpensslAsyncTask<T> {
    pub(crate) fn new(operation: T) -> Result<Self, ErrorStack> {
        let atomic_waker = Arc::new(AtomicWaker::new());
        let wait_ctx = AsyncWaitCtx::new()?;
        wait_ctx.set_callback(&atomic_waker)?;
        Ok(OpensslAsyncTask {
            job: ptr::null_mut(),
            wait_ctx,
            atomic_waker,
            action: Box::new(UnsafeCell::new(Action {
                operation,
                result: Err(anyhow!("not run yet")),
            })),
        })
    }

    fn poll_run(&mut self, cx: &mut Context<'_>) -> Poll<Result<T::Output, OpensslAsyncTaskError>> {
        let mut ret: c_int = 0;

        self.atomic_waker.register(cx.waker());

        loop {
            let mut param = self.action.get();
            let r = unsafe {
                ffi::ASYNC_start_job(
                    &mut self.job,
                    self.wait_ctx.as_ptr(),
                    &mut ret,
                    Some(start_job::<T>),
                    &mut param as *mut _ as *mut c_void,
                    mem::size_of::<*mut Action<T>>(),
                )
            };

            match r {
                ffi::ASYNC_ERR => return Poll::Ready(Err(ErrorStack::get().into())),
                ffi::ASYNC_NO_JOBS => {
                    // no available jobs, yield now and wake later
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
                        ffi::ASYNC_STATUS_OK => {
                            // submitted, wait for the callback
                            return Poll::Pending;
                        }
                        ffi::ASYNC_STATUS_EAGAIN => {
                            // engine busy, resume later
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
}

extern "C" fn start_job<T: AsyncOperation>(arg: *mut c_void) -> c_int {
    let p = unsafe { *(arg as *mut *mut Action<T>) };
    let action = unsafe { &mut *p };
    action.result = action.operation.run();
    0
}

impl<T> Future for OpensslAsyncTask<T>
where
    T: AsyncOperation + Unpin,
    T::Output: Unpin,
{
    type Output = Result<T::Output, OpensslAsyncTaskError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().poll_run(cx)
    }
}
