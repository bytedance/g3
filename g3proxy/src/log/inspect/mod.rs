/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use slog::Logger;

use g3_types::metrics::NodeName;

pub(crate) mod stream;

pub(crate) enum InspectSource {
    StreamInspection,
    TlsAlpn,
    StartTls,
    H2ExtendedConnect,
    HttpUpgrade,
}

impl InspectSource {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            InspectSource::StreamInspection => "stream inspection",
            InspectSource::TlsAlpn => "tls alpn",
            InspectSource::StartTls => "start tls",
            InspectSource::H2ExtendedConnect => "h2 extended connect",
            InspectSource::HttpUpgrade => "http upgrade",
        }
    }
}

pub(crate) fn get_logger(auditor_name: &NodeName) -> Option<Logger> {
    super::audit::get_logger(super::LOG_TYPE_INSPECT, auditor_name)
}
