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

use anyhow::anyhow;
use std::future::Future;
use std::io;
use std::os::fd::RawFd;
use std::pin::Pin;
use std::task::{ready, Context, Poll};
use std::{mem, ptr};

use foreign_types::ForeignType;
use libc::{c_int, c_void};
use openssl::error::ErrorStack;
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
    Operation(#[from] anyhow::Error),
}

pub struct OpensslAsyncTask<T> {
    job: *mut ffi::ASYNC_JOB,
    wait_ctx: AsyncWaitCtx,
    operation: T,
}

/// NOTE: OpensslAsyncTask in fact is not Send,
/// make sure you call it in a single threaded async runtime
unsafe impl<T: Send> Send for OpensslAsyncTask<T> {}

struct CallbackValue<'a, T: AsyncOperation> {
    op: &'a mut T,
    r: anyhow::Result<T::Output>,
}

impl<T: AsyncOperation> OpensslAsyncTask<T> {
    pub(crate) fn new(operation: T) -> Result<Self, ErrorStack> {
        let wait_ctx = AsyncWaitCtx::new()?;
        Ok(OpensslAsyncTask {
            job: ptr::null_mut(),
            wait_ctx,
            operation,
        })
    }

    fn poll_run(&mut self, cx: &mut Context<'_>) -> Poll<Result<T::Output, OpensslAsyncTaskError>> {
        let mut ret: c_int = 0;

        loop {
            let mut value = CallbackValue {
                op: &mut self.operation,
                r: Err(anyhow!("no result returned from the sync operation")),
            };

            let r = unsafe {
                ffi::ASYNC_start_job(
                    &mut self.job,
                    self.wait_ctx.as_ptr(),
                    &mut ret,
                    Some(start_job::<T>),
                    &mut value as *mut CallbackValue<T> as *mut c_void,
                    mem::size_of::<CallbackValue<T>>(),
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
                    let (add, del) = self.wait_ctx.get_changed_fds()?;
                    for fd in add {
                        self.operation.track_raw_fd(fd)?;
                    }
                    for fd in del {
                        self.operation.untrack_raw_fd(fd);
                    }
                    ready!(self.operation.poll_ready_fds(cx))?;
                }
                ffi::ASYNC_FINISH => {
                    return Poll::Ready(value.r.map_err(OpensslAsyncTaskError::Operation));
                }
                _ => unreachable!(),
            }
        }
    }
}

extern "C" fn start_job<T: AsyncOperation>(arg: *mut c_void) -> c_int {
    let mut task = ptr::NonNull::new(arg as *mut CallbackValue<T>).unwrap();
    let task = unsafe { task.as_mut() };
    task.r = task.op.run();
    0
}

impl<T> Future for OpensslAsyncTask<T>
where
    T: AsyncOperation + Unpin,
{
    type Output = Result<T::Output, OpensslAsyncTaskError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_run(cx)
    }
}
