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
use std::sync::Arc;

use g3_types::net::UpstreamAddr;

use super::RouteFailoverEscaper;
use crate::escape::ArcEscaper;
use crate::module::ftp_over_http::{BoxFtpConnectContext, FtpTaskRemoteControlStats};
use crate::serve::ServerTaskNotes;

struct NullStats {}

impl FtpTaskRemoteControlStats for NullStats {
    fn add_read_bytes(&self, _size: u64) {}

    fn add_write_bytes(&self, _size: u64) {}
}

struct FtpConnectFailoverContext {
    escaper: ArcEscaper,
}

impl FtpConnectFailoverContext {
    fn new(escaper: ArcEscaper) -> Self {
        FtpConnectFailoverContext { escaper }
    }

    async fn run(
        self,
        task_notes: &ServerTaskNotes,
        upstream: &UpstreamAddr,
    ) -> Result<BoxFtpConnectContext, BoxFtpConnectContext> {
        let mut ftp_ctx = self
            .escaper
            .new_ftp_connect_context(self.escaper.clone(), task_notes, upstream)
            .await;
        let null_stats = Arc::new(NullStats {});
        // try connect
        match ftp_ctx.new_control_connection(task_notes, null_stats).await {
            Ok(_) => Ok(ftp_ctx),
            Err(_) => Err(ftp_ctx),
        }
    }
}

impl RouteFailoverEscaper {
    pub(super) async fn new_ftp_connect_context_with_failover<'a>(
        &'a self,
        task_notes: &'a ServerTaskNotes,
        upstream: &'a UpstreamAddr,
    ) -> BoxFtpConnectContext {
        let primary_context = FtpConnectFailoverContext::new(self.primary_node.clone());
        let mut primary_task = pin!(primary_context.run(task_notes, upstream));

        match tokio::time::timeout(self.config.fallback_delay, &mut primary_task).await {
            Ok(Ok(ctx)) => {
                self.stats.add_request_passed();
                return ctx;
            }
            Ok(Err(_)) => {
                self.stats.add_request_passed(); // just return the ftp ctx on the standby escaper
                return self
                    .standby_node
                    .new_ftp_connect_context(self.standby_node.clone(), task_notes, upstream)
                    .await;
            }
            Err(_) => {}
        }

        let standby_context = FtpConnectFailoverContext::new(self.standby_node.clone());
        let standby_task = pin!(standby_context.run(task_notes, upstream));

        match futures_util::future::select_ok([primary_task, standby_task]).await {
            Ok((ctx, _left)) => {
                self.stats.add_request_passed();
                ctx
            }
            Err(ctx) => {
                self.stats.add_request_failed();
                ctx
            }
        }
    }
}
