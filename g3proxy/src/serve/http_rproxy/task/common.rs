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

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use slog::Logger;
use tokio::time::Instant;

use g3_daemon::server::ClientConnectionInfo;
use g3_io_ext::{IdleWheel, OptionalInterval};

use super::{HttpRProxyServerConfig, HttpRProxyServerStats};
use crate::escape::ArcEscaper;
use crate::serve::ServerQuitPolicy;

#[derive(Clone)]
pub(crate) struct CommonTaskContext {
    pub(crate) server_config: Arc<HttpRProxyServerConfig>,
    pub(crate) server_stats: Arc<HttpRProxyServerStats>,
    pub(crate) server_quit_policy: Arc<ServerQuitPolicy>,
    pub(crate) idle_wheel: Arc<IdleWheel>,
    pub(crate) escaper: ArcEscaper,
    pub(crate) cc_info: ClientConnectionInfo,
    pub(crate) task_logger: Option<Logger>,
}

impl CommonTaskContext {
    #[inline]
    pub(crate) fn client_addr(&self) -> SocketAddr {
        self.cc_info.client_addr()
    }

    #[inline]
    pub(crate) fn client_ip(&self) -> IpAddr {
        self.cc_info.client_ip()
    }

    #[inline]
    pub(crate) fn server_addr(&self) -> SocketAddr {
        self.cc_info.server_addr()
    }

    pub(super) fn log_flush_interval(&self) -> Option<Duration> {
        self.task_logger.as_ref()?;
        self.server_config.task_log_flush_interval
    }

    pub(super) fn get_log_interval(&self) -> OptionalInterval {
        self.log_flush_interval()
            .map(|log_interval| {
                let log_interval =
                    tokio::time::interval_at(Instant::now() + log_interval, log_interval);
                OptionalInterval::with(log_interval)
            })
            .unwrap_or_default()
    }
}
