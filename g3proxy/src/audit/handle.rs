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

use std::sync::Arc;

use slog::Logger;

use g3_dpi::{
    H1InterceptionConfig, H2InterceptionConfig, ImapInterceptionConfig, ProtocolInspectPolicy,
    ProtocolInspectionConfig, ProtocolPortMap, SmtpInterceptionConfig,
};
use g3_icap_client::reqmod::IcapReqmodClient;
use g3_icap_client::respmod::IcapRespmodClient;

use super::Auditor;
#[cfg(feature = "quic")]
use super::StreamDetourClient;
use crate::config::audit::AuditorConfig;
use crate::inspect::tls::TlsInterceptionContext;

pub(crate) struct AuditHandle {
    auditor_config: Arc<AuditorConfig>,
    server_tcp_portmap: Arc<ProtocolPortMap>,
    client_tcp_portmap: Arc<ProtocolPortMap>,
    tls_interception: Option<TlsInterceptionContext>,
    inspect_logger: Logger,
    intercept_logger: Logger,
    icap_reqmod_client: Option<IcapReqmodClient>,
    icap_respmod_client: Option<IcapRespmodClient>,
    #[cfg(feature = "quic")]
    stream_detour_client: Option<Arc<StreamDetourClient>>,
    pub(crate) h2_inspect_policy: ProtocolInspectPolicy,
    pub(crate) websocket_inspect_policy: ProtocolInspectPolicy,
    pub(crate) smtp_inspect_policy: ProtocolInspectPolicy,
    pub(crate) imap_inspect_policy: ProtocolInspectPolicy,
}

impl AuditHandle {
    pub(super) fn new(auditor: &Auditor) -> Self {
        let icap_reqmod_service = auditor
            .icap_reqmod_service
            .as_ref()
            .map(|c| IcapReqmodClient::new(c.clone()));
        let icap_respmod_service = auditor
            .icap_respmod_service
            .as_ref()
            .map(|c| IcapRespmodClient::new(c.clone()));
        AuditHandle {
            auditor_config: auditor.config.clone(),
            server_tcp_portmap: auditor.server_tcp_portmap.clone(),
            client_tcp_portmap: auditor.client_tcp_portmap.clone(),
            tls_interception: None,
            inspect_logger: crate::log::inspect::get_logger(auditor.config.name()),
            intercept_logger: crate::log::intercept::get_logger(auditor.config.name()),
            icap_reqmod_client: icap_reqmod_service,
            icap_respmod_client: icap_respmod_service,
            #[cfg(feature = "quic")]
            stream_detour_client: auditor.stream_detour_service.clone(),
            h2_inspect_policy: auditor.config.h2_inspect_policy.build(),
            websocket_inspect_policy: auditor.config.websocket_inspect_policy.build(),
            smtp_inspect_policy: auditor.config.smtp_inspect_policy.build(),
            imap_inspect_policy: auditor.config.imap_inspect_policy.build(),
        }
    }

    pub(super) fn set_tls_interception(&mut self, ctx: TlsInterceptionContext) {
        self.tls_interception = Some(ctx);
    }

    #[inline]
    pub(crate) fn inspect_logger(&self) -> &Logger {
        &self.inspect_logger
    }

    #[inline]
    pub(crate) fn intercept_logger(&self) -> &Logger {
        &self.intercept_logger
    }

    #[inline]
    pub(crate) fn protocol_inspection(&self) -> &ProtocolInspectionConfig {
        &self.auditor_config.protocol_inspection
    }

    #[inline]
    pub(crate) fn server_tcp_portmap(&self) -> Arc<ProtocolPortMap> {
        self.server_tcp_portmap.clone()
    }

    #[inline]
    pub(crate) fn client_tcp_portmap(&self) -> Arc<ProtocolPortMap> {
        self.client_tcp_portmap.clone()
    }

    #[inline]
    pub(crate) fn tls_interception(&self) -> Option<TlsInterceptionContext> {
        self.tls_interception.clone()
    }

    #[inline]
    pub(crate) fn log_uri_max_chars(&self) -> usize {
        self.auditor_config.log_uri_max_chars
    }

    #[inline]
    pub(crate) fn h1_interception(&self) -> &H1InterceptionConfig {
        &self.auditor_config.h1_interception
    }

    #[inline]
    pub(crate) fn h2_interception(&self) -> &H2InterceptionConfig {
        &self.auditor_config.h2_interception
    }

    #[inline]
    pub(crate) fn smtp_interception(&self) -> &SmtpInterceptionConfig {
        &self.auditor_config.smtp_interception
    }

    #[inline]
    pub(crate) fn imap_interception(&self) -> &ImapInterceptionConfig {
        &self.auditor_config.imap_interception
    }

    #[inline]
    pub(crate) fn icap_reqmod_client(&self) -> Option<&IcapReqmodClient> {
        self.icap_reqmod_client.as_ref()
    }

    #[inline]
    pub(crate) fn icap_respmod_client(&self) -> Option<&IcapRespmodClient> {
        self.icap_respmod_client.as_ref()
    }

    #[cfg(feature = "quic")]
    #[inline]
    pub(crate) fn stream_detour_client(&self) -> Option<&Arc<StreamDetourClient>> {
        self.stream_detour_client.as_ref()
    }

    pub(crate) fn do_task_audit(&self) -> bool {
        use rand::distr::Distribution;

        let mut rng = rand::rng();
        self.auditor_config.task_audit_ratio.sample(&mut rng)
    }
}
