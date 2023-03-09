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

use g3_io_ext::{LimitedBufReader, LimitedWriter, NilLimitedReaderStats};
use g3_types::net::OpensslTlsClientConfig;

use super::{DirectFixedEscaper, DirectFixedEscaperStats};
use crate::log::escape::tls_handshake::TlsApplication;
use crate::module::http_forward::{
    ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection, HttpForwardRemoteStatsWrapper,
};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectTaskNotes};
use crate::serve::ServerTaskNotes;

mod stats;
use stats::DirectHttpMixedRemoteStats;

mod reader;
mod writer;

use reader::DirectFixedHttpForwardReader;
use writer::DirectFixedHttpForwardWriter;

impl DirectFixedEscaper {
    pub(super) async fn http_forward_new_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        let stream = self.tcp_connect_to(tcp_notes, task_notes).await?;

        let (ups_r, ups_w) = stream.into_split();

        let mut w_wrapper_stats = DirectHttpMixedRemoteStats::new(&self.stats, &task_stats);
        let mut r_wrapper_stats = HttpForwardRemoteStatsWrapper::new(task_stats);
        let user_stats = self.fetch_user_upstream_io_stats(task_notes);
        w_wrapper_stats.push_user_io_stats_by_ref(&user_stats);
        r_wrapper_stats.push_user_io_stats(user_stats);

        let limit_config = &self.config.general.tcp_sock_speed_limit;
        let ups_r = LimitedBufReader::new(
            ups_r,
            limit_config.shift_millis,
            limit_config.max_south,
            self.stats.for_limited_reader(),
            r_wrapper_stats.into_reader(),
        );
        let ups_w = LimitedWriter::new(
            ups_w,
            limit_config.shift_millis,
            limit_config.max_north,
            w_wrapper_stats.into_writer(),
        );

        let writer = DirectFixedHttpForwardWriter::new(ups_w, Some(Arc::clone(&self.stats)));
        let reader = DirectFixedHttpForwardReader::new(ups_r);
        Ok((Box::new(writer), Box::new(reader)))
    }

    pub(super) async fn https_forward_new_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
        tls_config: &'a OpensslTlsClientConfig,
        tls_name: &'a str,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        let tls_stream = self
            .tls_connect_to(
                tcp_notes,
                task_notes,
                tls_config,
                tls_name,
                TlsApplication::HttpForward,
            )
            .await?;

        let (ups_r, ups_w) = tokio::io::split(tls_stream);

        // add task and user stats
        let mut wrapper_stats = HttpForwardRemoteStatsWrapper::new(task_stats);
        wrapper_stats.push_user_io_stats(self.fetch_user_upstream_io_stats(task_notes));
        let (ups_r_stats, ups_w_stats) = wrapper_stats.into_pair();

        let ups_r = LimitedBufReader::new_unlimited(
            ups_r,
            Arc::new(NilLimitedReaderStats::default()),
            ups_r_stats,
        );
        let ups_w = LimitedWriter::new_unlimited(ups_w, ups_w_stats);

        let writer = DirectFixedHttpForwardWriter::new(ups_w, None);
        let reader = DirectFixedHttpForwardReader::new(ups_r);
        Ok((Box::new(writer), Box::new(reader)))
    }
}
