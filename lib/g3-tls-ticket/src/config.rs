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

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;

use g3_types::net::{OpensslTicketKey, RollingTicketKey, RollingTicketer};

use super::TicketKeyUpdate;

#[derive(Clone)]
pub struct TlsTicketConfig {
    pub(crate) check_interval: Duration,
    pub(crate) local_lifetime: u32,
}

impl Default for TlsTicketConfig {
    fn default() -> Self {
        TlsTicketConfig {
            check_interval: Duration::from_secs(300),
            local_lifetime: 12 * 60 * 60, // 12h
        }
    }
}

impl TlsTicketConfig {
    pub fn build_and_spawn_updater(
        &self,
    ) -> anyhow::Result<Arc<RollingTicketer<OpensslTicketKey>>> {
        let initial_key = OpensslTicketKey::new_random(self.local_lifetime)
            .context("failed to create initial random key")?;
        let ticketer = Arc::new(RollingTicketer::new(initial_key));
        TicketKeyUpdate::new(self.clone(), ticketer.clone()).spawn_run();
        Ok(ticketer)
    }
}
