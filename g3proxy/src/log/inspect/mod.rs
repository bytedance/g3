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

use slog::Logger;

pub(crate) mod stream;

pub(crate) enum InspectSource {
    StreamInspection,
    TlsAlpn,
    H2ExtendedConnect,
    HttpUpgrade,
}

impl InspectSource {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            InspectSource::StreamInspection => "stream inspection",
            InspectSource::TlsAlpn => "tls alpn",
            InspectSource::H2ExtendedConnect => "h2 extended connect",
            InspectSource::HttpUpgrade => "http upgrade",
        }
    }
}

pub(crate) fn get_logger(auditor_name: &str) -> Logger {
    super::audit::get_logger(super::LOG_TYPE_INSPECT, auditor_name)
}
