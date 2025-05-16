/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use tokio::sync::Mutex;

mod bridge;

mod quit;
pub use quit::QuitActor;

mod upgrade;
pub use upgrade::UpgradeActor;

mod local;
pub use local::{DaemonController, UniqueController};

pub mod capnp;

static IO_MUTEX: Mutex<Option<Mutex<()>>> = Mutex::const_new(Some(Mutex::const_new(())));

pub(crate) async fn run_protected_io<F: Future>(future: F) -> Option<F::Output> {
    let outer = IO_MUTEX.lock().await;
    if let Some(inner) = &*outer {
        // io tasks that should avoid corrupt at exit should hold this lock
        let _guard = inner.lock();
        Some(future.await)
    } else {
        None
    }
}

pub(crate) async fn disable_protected_io() {
    let mut outer = IO_MUTEX.lock().await;
    if let Some(inner) = outer.take() {
        // wait all inner lock finish
        let _ = inner.lock().await;
    }
}
