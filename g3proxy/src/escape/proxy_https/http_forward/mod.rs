/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use g3_io_ext::{AsyncStream, LimitedBufReader, LimitedWriter, NilLimitedReaderStats};

use super::{ProxyHttpsEscaper, ProxyHttpsEscaperConfig};
use crate::log::escape::tls_handshake::TlsApplication;
use crate::module::http_forward::{
    ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection, HttpForwardTaskRemoteWrapperStats,
};
use crate::module::tcp_connect::{
    TcpConnectError, TcpConnectTaskConf, TcpConnectTaskNotes, TlsConnectTaskConf,
};
use crate::serve::ServerTaskNotes;

mod reader;
mod writer;

use reader::ProxyHttpsHttpForwardReader;
use writer::{ProxyHttpsHttpForwardWriter, ProxyHttpsHttpRequestWriter};

impl ProxyHttpsEscaper {
    pub(super) async fn http_forward_new_connection(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        let tls_stream = self
            .tls_handshake_to_remote(task_conf, tcp_notes, task_notes)
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

        let writer =
            ProxyHttpsHttpForwardWriter::new(ups_w, &self.config, task_conf.upstream.clone());
        let reader = ProxyHttpsHttpForwardReader::new(ups_r);
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

        let writer = ProxyHttpsHttpRequestWriter::new(ups_w, &self.config);
        let reader = ProxyHttpsHttpForwardReader::new(ups_r);
        Ok((Box::new(writer), Box::new(reader)))
    }
}
