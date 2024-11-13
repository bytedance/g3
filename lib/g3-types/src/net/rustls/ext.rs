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

use anyhow::anyhow;
#[cfg(feature = "aws-lc")]
use rustls::crypto::aws_lc_rs::Ticketer;
#[cfg(not(feature = "aws-lc"))]
use rustls::crypto::ring::Ticketer;
use rustls::server::{NoServerSessionStorage, ProducesTickets};
use rustls::{ClientConnection, HandshakeKind, ServerConfig, ServerConnection};

use super::{RustlsNoSessionTicketer, RustlsServerSessionCache};

pub trait RustlsConnectionExt {}

pub trait RustlsServerConnectionExt {
    fn session_reused(&self) -> bool;
}

impl RustlsServerConnectionExt for ServerConnection {
    fn session_reused(&self) -> bool {
        matches!(self.handshake_kind(), Some(HandshakeKind::Resumed))
    }
}

pub trait RustlsClientConnectionExt {
    fn session_reused(&self) -> bool;
}

impl RustlsClientConnectionExt for ClientConnection {
    fn session_reused(&self) -> bool {
        matches!(self.handshake_kind(), Some(HandshakeKind::Resumed))
    }
}

pub trait RustlsServerConfigExt {
    fn set_session_cache(&mut self, disable: bool);
    fn set_session_ticketer<T: ProducesTickets + 'static>(
        &mut self,
        enable: bool,
        ticketer: Option<Arc<T>>,
    ) -> anyhow::Result<()>;
}

impl RustlsServerConfigExt for ServerConfig {
    fn set_session_cache(&mut self, disable: bool) {
        if disable {
            self.session_storage = Arc::new(NoServerSessionStorage {});
        } else {
            self.session_storage = Arc::new(RustlsServerSessionCache::default());
        }
    }

    fn set_session_ticketer<T: ProducesTickets + 'static>(
        &mut self,
        enable: bool,
        ticketer: Option<Arc<T>>,
    ) -> anyhow::Result<()> {
        if enable {
            if let Some(ticketer) = ticketer {
                self.ticketer = ticketer;
            } else {
                self.ticketer = Ticketer::new()
                    .map_err(|e| anyhow!("failed to create session ticketer: {e}"))?;
            }
            self.send_tls13_tickets = 2;
        } else {
            self.ticketer = Arc::new(RustlsNoSessionTicketer {});
            self.send_tls13_tickets = 0;
        }
        Ok(())
    }
}
