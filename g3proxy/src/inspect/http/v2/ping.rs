/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use h2::{Ping, PingPong};
use tokio::sync::oneshot;

use g3_slog_types::{LtDuration, LtUpstreamAddr, LtUuid};
use g3_types::net::UpstreamAddr;

use super::H2ConcurrencyStats;
use crate::config::server::ServerConfig;
use crate::inspect::StreamInspectContext;

macro_rules! periodical_log {
    ($obj:tt, $rtt:expr, $($args:tt)+) => {
        if let Some(logger) = $obj.ctx.intercept_logger() {
            slog::info!(logger, $($args)+;
                "intercept_type" => "H2Ping",
                "task_id" => LtUuid($obj.ctx.server_task_id()),
                "depth" => $obj.ctx.inspection_depth,
                "upstream" => LtUpstreamAddr(&$obj.upstream),
                "rtt" => LtDuration($rtt),
                "total_sub_task" => $obj.stats.get_total_task(),
                "alive_sub_task" => $obj.stats.get_alive_task(),
            );
        }
    };
}

pub(super) struct H2PingTask<SC: ServerConfig> {
    ctx: StreamInspectContext<SC>,
    stats: Arc<H2ConcurrencyStats>,
    upstream: UpstreamAddr,
}

impl<SC: ServerConfig> H2PingTask<SC> {
    pub(super) fn new(
        ctx: StreamInspectContext<SC>,
        stats: Arc<H2ConcurrencyStats>,
        upstream: UpstreamAddr,
    ) -> Self {
        H2PingTask {
            ctx,
            stats,
            upstream,
        }
    }

    pub(super) async fn into_running(
        self,
        mut ping: PingPong,
        mut quit_receiver: oneshot::Receiver<()>,
    ) {
        let mut ping_interval = tokio::time::interval(self.ctx.h2_interception().ping_interval);
        loop {
            tokio::select! {
                _ = &mut quit_receiver => {
                    break;
                }
                time = ping_interval.tick() => {
                    match ping.ping(Ping::opaque()).await {
                        Ok(_) => {
                            periodical_log!(self, time.elapsed(), "ok");
                        }
                        Err(e) => {
                            periodical_log!(self, time.elapsed(), "{e}");
                            break;
                        }
                    }
                }
            }
        }
    }
}
