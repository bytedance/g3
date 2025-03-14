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

use g3_io_ext::{AsyncStream, LimitedBufReader, LimitedWriter, NilLimitedReaderStats};

use super::{ProxyHttpEscaper, ProxyHttpEscaperConfig, ProxyHttpEscaperStats};
use crate::log::escape::tls_handshake::TlsApplication;
use crate::module::http_forward::{
    ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection, HttpForwardRemoteWrapperStats,
    HttpForwardTaskRemoteWrapperStats,
};
use crate::module::tcp_connect::{
    TcpConnectError, TcpConnectTaskConf, TcpConnectTaskNotes, TlsConnectTaskConf,
};
use crate::serve::ServerTaskNotes;

mod reader;
mod writer;

use reader::ProxyHttpHttpForwardReader;
use writer::{ProxyHttpHttpForwardWriter, ProxyHttpHttpRequestWriter};

impl ProxyHttpEscaper {
    pub(super) async fn http_forward_new_connection(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        let stream = self
            .tcp_new_connection(task_conf, tcp_notes, task_notes)
            .await?;
        let (ups_r, mut ups_w) = stream.into_split();

        // add task and user stats
        let mut w_wrapper_stats =
            HttpForwardRemoteWrapperStats::new(self.stats.clone(), &task_stats);
        let mut r_wrapper_stats = HttpForwardTaskRemoteWrapperStats::new(task_stats);
        let user_stats = self.fetch_user_upstream_io_stats(task_notes);
        w_wrapper_stats.push_user_io_stats_by_ref(&user_stats);
        r_wrapper_stats.push_user_io_stats(user_stats);

        ups_w.reset_stats(Arc::new(w_wrapper_stats));
        let ups_r = LimitedBufReader::new_directed(ups_r, Arc::new(r_wrapper_stats));

        let writer = ProxyHttpHttpForwardWriter::new(
            ups_w,
            Some(Arc::clone(&self.stats)),
            &self.config,
            task_conf.upstream.clone(),
        );
        let reader = ProxyHttpHttpForwardReader::new(ups_r);
        Ok((Box::new(writer), Box::new(reader)))
    }

    pub(super) async fn https_forward_new_connection(
        &self,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        let tls_stream = self
            .http_connect_tls_connect_to(
                task_conf,
                tcp_notes,
                task_notes,
                TlsApplication::HttpForward,
            )
            .await?;

        let (ups_r, ups_w) = tls_stream.into_split();

        // add task and user stats
        let mut wrapper_stats = HttpForwardTaskRemoteWrapperStats::new(task_stats);
        wrapper_stats.push_user_io_stats(self.fetch_user_upstream_io_stats(task_notes));
        let wrapper_stats = Arc::new(wrapper_stats);

        let ups_r = LimitedBufReader::new_unlimited(
            ups_r,
            Arc::new(NilLimitedReaderStats::default()),
            wrapper_stats.clone(),
        );
        let ups_w = LimitedWriter::new(ups_w, wrapper_stats);

        let writer = ProxyHttpHttpRequestWriter::new(ups_w, None, &self.config);
        let reader = ProxyHttpHttpForwardReader::new(ups_r);
        Ok((Box::new(writer), Box::new(reader)))
    }
}
