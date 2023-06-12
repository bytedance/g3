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

use tokio::task::JoinSet;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_types::net::UpstreamAddr;

use super::RouteFailoverEscaper;
use crate::escape::ArcEscaper;
use crate::module::tcp_connect::{TcpConnectError, TcpConnectResult, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

pub struct TcpConnectFailoverContext {
    tcp_notes: TcpConnectTaskNotes,
    task_notes: ServerTaskNotes,
    connect_result: TcpConnectResult,
}

impl TcpConnectFailoverContext {
    fn new(upstream: &UpstreamAddr, task_notes: &ServerTaskNotes) -> Self {
        let tcp_notes = TcpConnectTaskNotes::new(upstream.clone());
        TcpConnectFailoverContext {
            tcp_notes,
            task_notes: task_notes.dup_for_read(),
            connect_result: Err(TcpConnectError::NoAddressConnected),
        }
    }

    async fn run(
        mut self,
        escaper: ArcEscaper,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> Self {
        let r = escaper
            .tcp_setup_connection(&mut self.tcp_notes, &self.task_notes, task_stats)
            .await;
        self.connect_result = r;
        self
    }
}

impl RouteFailoverEscaper {
    pub(super) async fn tcp_setup_connection_with_failover<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> TcpConnectResult {
        let mut c_set = JoinSet::new();

        let primary_context = TcpConnectFailoverContext::new(&tcp_notes.upstream, task_notes);
        c_set.spawn(primary_context.run(self.primary_node.clone(), task_stats.clone()));

        let mut error = TcpConnectError::EscaperNotUsable;

        match tokio::time::timeout(self.config.fallback_delay, c_set.join_next()).await {
            Ok(Some(Ok(d))) => match d.connect_result {
                Ok(c) => {
                    self.stats.add_request_passed();
                    tcp_notes.fill_generated(&d.tcp_notes);
                    return Ok(c);
                }
                Err(e) => error = e,
            },
            Ok(Some(Err(_))) => {
                return Err(TcpConnectError::InternalServerError(
                    "failed to join tcp connect task",
                ))
            }
            Ok(None) => {
                unreachable!()
            }
            Err(_) => {} // Timed out, now try standby
        }

        let standby_context = TcpConnectFailoverContext::new(&tcp_notes.upstream, task_notes);
        c_set.spawn(standby_context.run(self.standby_node.clone(), task_stats));

        while let Some(r) = c_set.join_next().await {
            match r {
                Ok(d) => match d.connect_result {
                    Ok(c) => {
                        self.stats.add_request_passed();
                        tcp_notes.fill_generated(&d.tcp_notes);
                        return Ok(c);
                    }
                    Err(e) => error = e,
                },
                Err(_) => {
                    return Err(TcpConnectError::InternalServerError(
                        "failed to join tcp connect task",
                    ));
                }
            }
        }

        Err(error)
    }
}
