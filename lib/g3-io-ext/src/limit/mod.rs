/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Mutex;
use std::thread::JoinHandle;

use tokio::runtime::{Handle, RuntimeMetrics};
use tokio::sync::oneshot;

static SCHEDULE_RUNTIME: Mutex<Option<Handle>> = Mutex::new(None);
static THREAD_QUIT_SENDER: Mutex<Option<oneshot::Sender<()>>> = Mutex::new(None);
static THREAD_JOIN_HANDLE: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GlobalLimitGroup {
    Server,
    User,
    UserSite,
}

mod datagram;
pub use datagram::{DatagramLimitAction, DatagramLimiter, GlobalDatagramLimit};

mod stream;
pub use stream::{GlobalStreamLimit, StreamLimitAction, StreamLimiter};

mod fixed_window;
pub use fixed_window::{LocalDatagramLimiter, LocalStreamLimiter, ThreadedCountLimiter};

mod token_bucket;
pub use token_bucket::{GlobalDatagramLimiter, GlobalStreamLimiter};

pub async fn spawn_limit_schedule_runtime() -> Option<RuntimeMetrics> {
    let (quit_sender, quit_receiver) = oneshot::channel();
    set_thread_quit_sender(quit_sender);

    let (rt_handle_sender, rt_handle_receiver) = oneshot::channel();
    let Ok(handle) = std::thread::Builder::new()
        .name("limit-schedule".to_string())
        .spawn(move || {
            let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
            else {
                return;
            };

            if rt_handle_sender.send(rt.handle().clone()).is_ok() {
                let _ = rt.block_on(quit_receiver);
            }
        })
    else {
        return None;
    };
    set_thread_join_handle(handle);
    if let Ok(handle) = rt_handle_receiver.await {
        set_limit_schedule_rt_handle(handle.clone());
        Some(handle.metrics())
    } else {
        None
    }
}

pub fn close_limit_schedule_runtime() {
    let mut lock = THREAD_QUIT_SENDER.lock().unwrap();
    if let Some(sender) = lock.take() {
        let _ = sender.send(());
    }
    drop(lock);

    let mut lock = THREAD_JOIN_HANDLE.lock().unwrap();
    if let Some(join_handle) = lock.take() {
        let _ = join_handle.join();
    }
}

fn set_thread_quit_sender(sender: oneshot::Sender<()>) {
    let mut lock = THREAD_QUIT_SENDER.lock().unwrap();
    *lock = Some(sender);
}

fn set_thread_join_handle(handle: JoinHandle<()>) {
    let mut lock = THREAD_JOIN_HANDLE.lock().unwrap();
    *lock = Some(handle);
}

fn set_limit_schedule_rt_handle(handle: Handle) {
    let mut lock = SCHEDULE_RUNTIME.lock().unwrap();
    *lock = Some(handle);
}

pub fn get_limit_schedule_rt_handle() -> Option<Handle> {
    let lock = SCHEDULE_RUNTIME.lock().unwrap();
    lock.as_ref().cloned()
}
