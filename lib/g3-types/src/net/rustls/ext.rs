/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

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
                set_default_session_ticketer(self)?;
            }
        } else {
            self.ticketer = Arc::new(RustlsNoSessionTicketer {});
            self.send_tls13_tickets = 0;
        }
        Ok(())
    }
}

#[cfg(any(feature = "rustls-aws-lc", feature = "rustls-aws-lc-fips"))]
fn set_default_session_ticketer(config: &mut ServerConfig) -> anyhow::Result<()> {
    use anyhow::anyhow;

    config.ticketer = rustls::crypto::aws_lc_rs::Ticketer::new()
        .map_err(|e| anyhow!("failed to create session ticketer: {e}"))?;
    Ok(())
}

#[cfg(feature = "rustls-ring")]
fn set_default_session_ticketer(config: &mut ServerConfig) -> anyhow::Result<()> {
    use anyhow::anyhow;

    config.ticketer = rustls::crypto::ring::Ticketer::new()
        .map_err(|e| anyhow!("failed to create session ticketer: {e}"))?;
    Ok(())
}

#[cfg(not(any(
    feature = "rustls-aws-lc",
    feature = "rustls-aws-lc-fips",
    feature = "rustls-ring"
)))]
fn set_default_session_ticketer(config: &mut ServerConfig) -> anyhow::Result<()> {
    config.send_tls13_tickets = 0;
    Ok(())
}
