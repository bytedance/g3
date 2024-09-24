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

use std::sync::OnceLock;

use log::warn;
use tokio::runtime::Handle;

pub mod config;
pub mod worker;

pub mod metrics;

static MAIN_HANDLE: OnceLock<Handle> = OnceLock::new();

pub fn main_handle() -> Option<&'static Handle> {
    MAIN_HANDLE.get()
}

pub fn set_main_handle() {
    let handle = Handle::current();
    metrics::add_tokio_stats(handle.metrics(), "main".to_string());
    if MAIN_HANDLE.set(handle).is_err() {
        warn!("main handle has already been set");
    }
}
