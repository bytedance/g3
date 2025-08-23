/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use openssl::error::ErrorStack;

use crate::ffi;

mod wait_ctx;
pub use wait_ctx::AsyncWaitCtx;

mod task;
pub use task::{AsyncOperation, OpensslAsyncTask, SyncOperation};

mod tokio_op;
pub use tokio_op::TokioAsyncOperation;

pub fn async_is_capable() -> bool {
    let capable = unsafe { ffi::ASYNC_is_capable() };
    capable == 1
}

pub fn async_thread_init(max_size: usize, init_size: usize) -> Result<(), ErrorStack> {
    let r = unsafe { ffi::ASYNC_init_thread(max_size, init_size) };
    if r == 1 {
        Ok(())
    } else {
        Err(ErrorStack::get())
    }
}

pub fn async_thread_cleanup() {
    unsafe { ffi::ASYNC_cleanup_thread() }
}
