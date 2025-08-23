/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_statsd_client::StatsdClient;

use crate::FrontendStats;

pub(crate) fn emit_stats(client: &mut StatsdClient, s: &FrontendStats) {
    macro_rules! emit_count {
        ($take:ident, $name:literal) => {
            let v = s.$take();
            client.count(concat!("frontend.", $name), v).send();
        };
    }

    emit_count!(take_request_total, "request_total");
    emit_count!(take_request_invalid, "request_invalid");
    emit_count!(take_response_total, "response_total");
    emit_count!(take_response_fail, "response_fail");
}
