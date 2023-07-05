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

use openssl::error::ErrorStack;

mod ffi;

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
