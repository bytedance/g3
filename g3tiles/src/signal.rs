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

use log::{error, info, warn};
use once_cell::sync::Lazy;
use tokio::signal::unix::SignalKind;
use tokio::sync::Mutex;

use g3_signal::{ActionSignal, SigResult};

static RELOAD_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

fn do_quit(_: u32) -> SigResult {
    info!("got quit signal");
    tokio::spawn(async { crate::control::UniqueController::abort_immediately().await });
    SigResult::Break
}

fn go_offline(_: u32) -> SigResult {
    info!("got offline signal");
    tokio::spawn(async { crate::control::DaemonController::abort().await });
    SigResult::Break
}

fn call_reload(_: u32) -> SigResult {
    info!("got reload signal");
    let f = async { do_reload().await };
    tokio::spawn(f);
    SigResult::Continue
}

async fn do_reload() {
    let _guard = RELOAD_MUTEX.lock().await;
    info!("reloading config");

    if let Err(e) = crate::config::reload().await {
        warn!("error reloading config: {e:?}");
        warn!("reload aborted");
    }

    /*
    if let Err(e) = crate::resolve::spawn_all().await {
        error!("failed to reload all resolvers: {e:?}");
    }
    if let Err(e) = crate::escape::load_all().await {
        error!("failed to reload all escapers: {e:?}");
    }
    if let Err(e) = crate::auth::load_all().await {
        error!("failed to reload all user groups: {e:?}");
    }
    */
    if let Err(e) = crate::serve::spawn_all().await {
        error!("failed to reload all servers: {e:?}");
    }

    info!("reload finished");
}

pub fn setup_and_spawn() -> anyhow::Result<()> {
    tokio::spawn(ActionSignal::new(SignalKind::quit(), &do_quit)?);
    tokio::spawn(ActionSignal::new(SignalKind::interrupt(), &do_quit)?);
    tokio::spawn(ActionSignal::new(SignalKind::terminate(), &go_offline)?);
    tokio::spawn(ActionSignal::new(SignalKind::hangup(), &call_reload)?);
    Ok(())
}
