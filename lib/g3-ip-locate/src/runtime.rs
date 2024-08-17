/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use std::sync::Mutex;
use std::thread::JoinHandle;

use tokio::runtime::{Handle, RuntimeMetrics};
use tokio::sync::oneshot;

static SCHEDULE_RUNTIME: Mutex<Option<Handle>> = Mutex::new(None);
static THREAD_QUIT_SENDER: Mutex<Option<oneshot::Sender<()>>> = Mutex::new(None);
static THREAD_JOIN_HANDLE: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

pub async fn spawn_ip_locate_runtime() -> Option<RuntimeMetrics> {
    let (quit_sender, quit_receiver) = oneshot::channel();
    set_thread_quit_sender(quit_sender);

    let (rt_handle_sender, rt_handle_receiver) = oneshot::channel();
    let Ok(handle) = std::thread::Builder::new()
        .name("ip-locate".to_string())
        .spawn(move || {
            let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
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
        set_ip_locate_rt_handle(handle.clone());
        Some(handle.metrics())
    } else {
        None
    }
}

pub fn close_ip_locate_runtime() {
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

fn set_ip_locate_rt_handle(handle: Handle) {
    let mut lock = SCHEDULE_RUNTIME.lock().unwrap();
    *lock = Some(handle);
}

pub fn get_ip_locate_rt_handle() -> Option<Handle> {
    let lock = SCHEDULE_RUNTIME.lock().unwrap();
    lock.as_ref().cloned()
}
