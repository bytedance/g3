/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::Context;

use g3_dpi::ProtocolPortMap;
use g3_icap_client::IcapServiceClient;
use g3_types::metrics::NodeName;
use g3_types::net::{OpensslTicketKey, RollingTicketer};

use crate::config::audit::AuditorConfig;
use crate::inspect::tls::TlsInterceptionContext;

mod ops;
pub use ops::load_all;
pub(crate) use ops::reload;

mod registry;
pub(crate) use registry::{get_names, get_or_insert_default};

mod handle;
pub(crate) use handle::AuditHandle;

#[cfg(feature = "quic")]
mod detour;
#[cfg(feature = "quic")]
pub(crate) use detour::DetourAction;
#[cfg(feature = "quic")]
use detour::StreamDetourClient;

pub(crate) struct Auditor {
    config: Arc<AuditorConfig>,
    server_tcp_portmap: Arc<ProtocolPortMap>,
    client_tcp_portmap: Arc<ProtocolPortMap>,
    tls_rolling_ticketer: Option<Arc<RollingTicketer<OpensslTicketKey>>>,
    icap_reqmod_service: Option<Arc<IcapServiceClient>>,
    icap_respmod_service: Option<Arc<IcapServiceClient>>,
    #[cfg(feature = "quic")]
    stream_detour_service: Option<Arc<StreamDetourClient>>,
}

impl Auditor {
    fn new_no_config(name: &NodeName) -> Arc<Self> {
        let config = AuditorConfig::empty(name);
        let server_tcp_portmap = Arc::new(config.server_tcp_portmap.clone());
        let client_tcp_portmap = Arc::new(config.client_tcp_portmap.clone());
        let auditor = Auditor {
            config: Arc::new(config),
            server_tcp_portmap,
            client_tcp_portmap,
            tls_rolling_ticketer: None,
            icap_reqmod_service: None,
            icap_respmod_service: None,
            #[cfg(feature = "quic")]
            stream_detour_service: None,
        };
        Arc::new(auditor)
    }

    fn new_with_config(config: AuditorConfig) -> anyhow::Result<Arc<Self>> {
        let server_tcp_portmap = Arc::new(config.server_tcp_portmap.clone());
        let client_tcp_portmap = Arc::new(config.client_tcp_portmap.clone());
        let tls_rolling_ticketer = if let Some(c) = &config.tls_ticketer {
            let ticketer = c
                .build_and_spawn_updater()
                .context("failed to create tls rolling ticketer")?;
            Some(ticketer)
        } else {
            None
        };
        let mut auditor = Auditor {
            config: Arc::new(config),
            server_tcp_portmap,
            client_tcp_portmap,
            tls_rolling_ticketer,
            icap_reqmod_service: None,
            icap_respmod_service: None,
            #[cfg(feature = "quic")]
            stream_detour_service: None,
        };
        auditor.set_agent_clients()?;
        Ok(Arc::new(auditor))
    }

    fn reload(&self, config: AuditorConfig) -> anyhow::Result<Arc<Self>> {
        let server_tcp_portmap = Arc::new(config.server_tcp_portmap.clone());
        let client_tcp_portmap = Arc::new(config.client_tcp_portmap.clone());
        let tls_rolling_ticketer = if self.config.tls_ticketer.eq(&config.tls_ticketer) {
            self.tls_rolling_ticketer.clone()
        } else if let Some(c) = &config.tls_ticketer {
            let ticketer = c
                .build_and_spawn_updater()
                .context("failed to create tls rolling ticketer")?;
            Some(ticketer)
        } else {
            None
        };
        let mut auditor = Auditor {
            config: Arc::new(config),
            server_tcp_portmap,
            client_tcp_portmap,
            tls_rolling_ticketer,
            icap_reqmod_service: None,
            icap_respmod_service: None,
            #[cfg(feature = "quic")]
            stream_detour_service: None,
        };
        auditor.set_agent_clients()?;
        Ok(Arc::new(auditor))
    }

    fn set_agent_clients(&mut self) -> anyhow::Result<()> {
        if let Some(c) = self.config.icap_reqmod_service.clone() {
            self.icap_reqmod_service = Some(Arc::new(
                IcapServiceClient::new(c).context("failed to create ICAP REQMOD client")?,
            ));
        }
        if let Some(c) = self.config.icap_respmod_service.clone() {
            self.icap_respmod_service = Some(Arc::new(
                IcapServiceClient::new(c).context("failed to create ICAP RESPMOD client")?,
            ));
        }
        #[cfg(feature = "quic")]
        if let Some(c) = self.config.stream_detour_service.clone() {
            let client = StreamDetourClient::new(c)?;
            self.stream_detour_service = Some(Arc::new(client));
        }
        Ok(())
    }

    pub(crate) fn build_handle(&self) -> anyhow::Result<Arc<AuditHandle>> {
        let mut handle = AuditHandle::new(self);

        if let Some(cert_agent_config) = &self.config.tls_cert_agent {
            let cert_agent = cert_agent_config
                .spawn_cert_agent()
                .context("failed to spawn cert generator task")?;
            let client_config = self
                .config
                .tls_interception_client
                .build()
                .context("failed to build tls client config")?;
            let server_config = self
                .config
                .tls_interception_server
                .build_with_ticketer(self.tls_rolling_ticketer.as_ref())
                .context("failed to build tls server config")?;
            let ctx = TlsInterceptionContext::new(
                cert_agent,
                client_config,
                server_config,
                self.config.tls_stream_dump,
            )?;
            handle.set_tls_interception(ctx);
        }

        Ok(Arc::new(handle))
    }
}

#[derive(Clone, Default)]
pub(crate) struct AuditContext {
    handle: Option<Arc<AuditHandle>>,
}

impl AuditContext {
    pub(crate) fn new(handle: Option<Arc<AuditHandle>>) -> Self {
        AuditContext { handle }
    }

    pub(crate) fn set_handle(&mut self, handle: Arc<AuditHandle>) {
        self.handle = Some(handle);
    }

    pub(crate) fn handle(&self) -> Option<&Arc<AuditHandle>> {
        self.handle.as_ref()
    }

    pub(crate) fn check_take_handle(&mut self) -> Option<Arc<AuditHandle>> {
        self.handle.take().filter(|handle| handle.do_task_audit())
    }
}
