/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_daemon::metrics::TAG_KEY_QUANTILE;
use g3_histogram::HistogramStats;
use g3_statsd_client::StatsdClient;

use crate::BackendStats;

pub(crate) fn emit_stats(client: &mut StatsdClient, s: &BackendStats) {
    macro_rules! emit_count {
        ($take:ident, $name:literal) => {
            let v = s.$take();
            client.count(concat!("backend.", $name), v).send();
        };
    }

    emit_count!(take_refresh_total, "refresh_total");
    emit_count!(take_refresh_ok, "refresh_ok");
    emit_count!(take_request_total, "request_total");
    emit_count!(take_request_ok, "request_ok");
}

pub(crate) fn emit_duration_stats(client: &mut StatsdClient, s: &HistogramStats) {
    s.foreach_stat(|_, qs, v| {
        client
            .gauge_float("backend.request_duration", v)
            .with_tag(TAG_KEY_QUANTILE, qs)
            .send();
    });
}
