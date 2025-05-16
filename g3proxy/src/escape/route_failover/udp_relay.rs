/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::pin::pin;

use anyhow::anyhow;

use super::RouteFailoverEscaper;
use crate::escape::ArcEscaper;
use crate::module::udp_relay::{
    ArcUdpRelayTaskRemoteStats, UdpRelaySetupError, UdpRelaySetupResult, UdpRelayTaskConf,
    UdpRelayTaskNotes,
};
use crate::serve::ServerTaskNotes;

struct UdpRelayFailoverContext {
    udp_notes: UdpRelayTaskNotes,
    setup_result: UdpRelaySetupResult,
}

impl UdpRelayFailoverContext {
    fn new() -> Self {
        UdpRelayFailoverContext {
            udp_notes: UdpRelayTaskNotes::default(),
            setup_result: Err(UdpRelaySetupError::EscaperNotUsable(anyhow!(
                "no udp set relay called yet"
            ))),
        }
    }

    async fn run(
        mut self,
        escaper: &ArcEscaper,
        task_conf: &UdpRelayTaskConf<'_>,
        task_notes: &ServerTaskNotes,
        task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> Result<Self, Self> {
        match escaper
            .udp_setup_relay(task_conf, &mut self.udp_notes, task_notes, task_stats)
            .await
        {
            Ok(c) => {
                self.setup_result = Ok(c);
                Ok(self)
            }
            Err(e) => {
                self.setup_result = Err(e);
                Err(self)
            }
        }
    }
}

impl RouteFailoverEscaper {
    pub(super) async fn udp_setup_relay_with_failover(
        &self,
        task_conf: &UdpRelayTaskConf<'_>,
        udp_notes: &mut UdpRelayTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> UdpRelaySetupResult {
        let primary_context = UdpRelayFailoverContext::new();
        let mut primary_task = pin!(primary_context.run(
            &self.primary_node,
            task_conf,
            task_notes,
            task_stats.clone()
        ));

        match tokio::time::timeout(self.config.fallback_delay, &mut primary_task).await {
            Ok(Ok(ctx)) => {
                self.stats.add_request_passed();
                udp_notes.clone_from(&ctx.udp_notes);
                return ctx.setup_result;
            }
            Ok(Err(_)) => {
                return match self
                    .standby_node
                    .udp_setup_relay(task_conf, udp_notes, task_notes, task_stats)
                    .await
                {
                    Ok(c) => {
                        self.stats.add_request_passed();
                        Ok(c)
                    }
                    Err(e) => {
                        self.stats.add_request_failed();
                        Err(e)
                    }
                };
            }
            Err(_) => {}
        }

        let standby_context = UdpRelayFailoverContext::new();
        let standby_task =
            pin!(standby_context.run(&self.standby_node, task_conf, task_notes, task_stats));

        match futures_util::future::select_ok([primary_task, standby_task]).await {
            Ok((ctx, _left)) => {
                self.stats.add_request_passed();
                udp_notes.clone_from(&ctx.udp_notes);
                ctx.setup_result
            }
            Err(ctx) => {
                self.stats.add_request_failed();
                udp_notes.clone_from(&ctx.udp_notes);
                ctx.setup_result
            }
        }
    }
}
