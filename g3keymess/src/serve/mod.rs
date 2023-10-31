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

mod stats;
pub(crate) use stats::{
    KeyServerDurationRecorder, KeyServerDurationStats, KeyServerRequestSnapshot,
    KeyServerRequestStats, KeyServerSnapshot, KeyServerStats,
};

mod error;
pub(crate) use error::ServerTaskError;

mod server;
pub(crate) use server::KeyServer;

mod task;
use task::{KeylessTask, KeylessTaskContext};

mod runtime;
use runtime::KeyServerRuntime;

mod registry;
pub(crate) use registry::{foreach_online as foreach_server, get_names};

mod ops;
pub use ops::{create_all_stopped, spawn_all, spawn_offline_clean, start_all_stopped};
pub(crate) use ops::{get_server, stop_all, wait_all_tasks};

#[derive(Clone)]
pub(crate) enum ServerReloadCommand {
    QuitRuntime,
}
