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
