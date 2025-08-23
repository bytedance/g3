/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use g3_io_ext::{AsyncStream, LimitedBufReader, LimitedWriter, NilLimitedReaderStats};

use super::{
    ProxyFloatEscaper, ProxyFloatEscaperStats, ProxyFloatHttpPeer, ProxyFloatHttpPeerSharedConfig,
};
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

pub(crate) use reader::HttpPeerHttpForwardReader;
use writer::{HttpPeerHttpForwardWriter, HttpPeerHttpRequestWriter};

impl ProxyFloatHttpPeer {
    pub(super) async fn http_forward_new_connection(
        &self,
        escaper: &ProxyFloatEscaper,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        let stream = escaper
            .tcp_new_connection(self, task_conf, tcp_notes, task_notes)
            .await?;
        let (ups_r, mut ups_w) = stream.into_split();

        let mut w_wrapper_stats =
            HttpForwardRemoteWrapperStats::new(escaper.stats.clone(), &task_stats);
        let mut r_wrapper_stats = HttpForwardTaskRemoteWrapperStats::new(task_stats);
        let user_stats = escaper.fetch_user_upstream_io_stats(task_notes);
        w_wrapper_stats.push_user_io_stats_by_ref(&user_stats);
        r_wrapper_stats.push_user_io_stats(user_stats);

        ups_w.reset_stats(Arc::new(w_wrapper_stats));
        let ups_r = LimitedBufReader::new_directed(ups_r, Arc::new(r_wrapper_stats));

        let writer = HttpPeerHttpForwardWriter::new(
            ups_w,
            Some(escaper.stats.clone()),
            &self.shared_config,
            task_conf.upstream.clone(),
        );
        let reader = HttpPeerHttpForwardReader::new(ups_r);
        Ok((Box::new(writer), Box::new(reader)))
    }

    pub(super) async fn https_forward_new_connection(
        &self,
        escaper: &ProxyFloatEscaper,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        let tls_stream = self
            .http_connect_tls_connect_to(
                escaper,
                task_conf,
                tcp_notes,
                task_notes,
                TlsApplication::HttpForward,
            )
            .await?;

        let (ups_r, ups_w) = tls_stream.into_split();

        // add task and user stats
        let mut wrapper_stats = HttpForwardTaskRemoteWrapperStats::new(task_stats);
        wrapper_stats.push_user_io_stats(escaper.fetch_user_upstream_io_stats(task_notes));
        let wrapper_stats = Arc::new(wrapper_stats);

        let ups_r = LimitedBufReader::new_unlimited(
            ups_r,
            Arc::new(NilLimitedReaderStats::default()),
            wrapper_stats.clone(),
        );
        let ups_w = LimitedWriter::new(ups_w, wrapper_stats);

        let writer = HttpPeerHttpRequestWriter::new(ups_w, None, &self.shared_config);
        let reader = HttpPeerHttpForwardReader::new(ups_r);
        Ok((Box::new(writer), Box::new(reader)))
    }
}
