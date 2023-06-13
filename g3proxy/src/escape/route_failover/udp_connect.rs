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

use std::pin::pin;

use super::RouteFailoverEscaper;
use crate::escape::ArcEscaper;
use crate::module::udp_connect::{
    ArcUdpConnectTaskRemoteStats, UdpConnectError, UdpConnectResult, UdpConnectTaskNotes,
};
use crate::serve::ServerTaskNotes;

struct UdpConnectFailoverContext {
    udp_notes: UdpConnectTaskNotes,
    connect_result: UdpConnectResult,
}

impl UdpConnectFailoverContext {
    fn new(udp_notes: &UdpConnectTaskNotes) -> Self {
        UdpConnectFailoverContext {
            udp_notes: udp_notes.dup_as_new(),
            connect_result: Err(UdpConnectError::EscaperNotUsable),
        }
    }

    async fn run(
        mut self,
        escaper: &ArcEscaper,
        task_notes: &ServerTaskNotes,
        task_stats: ArcUdpConnectTaskRemoteStats,
    ) -> Result<Self, Self> {
        match escaper
            .udp_setup_connection(&mut self.udp_notes, task_notes, task_stats)
            .await
        {
            Ok(c) => {
                self.connect_result = Ok(c);
                Ok(self)
            }
            Err(e) => {
                self.connect_result = Err(e);
                Err(self)
            }
        }
    }
}

impl RouteFailoverEscaper {
    pub(super) async fn udp_setup_connection_with_failover<'a>(
        &'a self,
        udp_notes: &'a mut UdpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcUdpConnectTaskRemoteStats,
    ) -> UdpConnectResult {
        let primary_context = UdpConnectFailoverContext::new(udp_notes);
        let mut primary_task =
            pin!(primary_context.run(&self.primary_node, task_notes, task_stats.clone()));

        match tokio::time::timeout(self.config.fallback_delay, &mut primary_task).await {
            Ok(Ok(ctx)) => {
                self.stats.add_request_passed();
                udp_notes.fill_generated(&ctx.udp_notes);
                return ctx.connect_result;
            }
            Ok(Err(_)) => {
                return match self
                    .standby_node
                    .udp_setup_connection(udp_notes, task_notes, task_stats)
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
                }
            }
            Err(_) => {}
        }

        let standby_context = UdpConnectFailoverContext::new(udp_notes);
        let standby_task = pin!(standby_context.run(&self.standby_node, task_notes, task_stats));

        match futures_util::future::select_ok([primary_task, standby_task]).await {
            Ok((ctx, _left)) => {
                self.stats.add_request_passed();
                udp_notes.fill_generated(&ctx.udp_notes);
                ctx.connect_result
            }
            Err(ctx) => {
                self.stats.add_request_failed();
                udp_notes.fill_generated(&ctx.udp_notes);
                ctx.connect_result
            }
        }
    }
}
