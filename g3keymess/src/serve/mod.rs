/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod stats;
pub(crate) use stats::{
    KeyServerAliveTaskGuard, KeyServerDurationRecorder, KeyServerDurationStats,
    KeyServerRequestSnapshot, KeyServerRequestStats, KeyServerSnapshot, KeyServerStats,
};

mod error;
pub(crate) use error::ServerTaskError;

mod server;
pub(crate) use server::KeyServer;

mod task;
use task::{KeylessTask, KeylessTaskContext};
pub(crate) use task::{RequestProcessContext, WrappedKeylessRequest, WrappedKeylessResponse};

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
