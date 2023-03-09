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

use g3_types::log::AsyncLogConfig;

mod async_streamer;

mod backend;
mod format;
mod types;
mod util;

pub use types::{Facility, Severity};

use async_streamer::AsyncSyslogStreamer;

pub use backend::SyslogBackendBuilder;

use format::BoxSyslogFormatter;
pub use format::SyslogFormatterKind;

pub struct SyslogHeader {
    pub facility: Facility,
    pub hostname: Option<String>,
    pub process: String,
    pub pid: u32,
}

#[derive(Clone, Debug)]
pub struct SyslogBuilder {
    ident: String,
    facility: Facility,
    backend: SyslogBackendBuilder,
    format: SyslogFormatterKind,
    emit_hostname: bool,
    append_report_ts: bool,
}

impl Default for SyslogBuilder {
    fn default() -> Self {
        SyslogBuilder::with_ident(String::new())
    }
}

impl SyslogBuilder {
    pub fn with_ident(ident: String) -> Self {
        SyslogBuilder {
            ident,
            facility: Facility::User,
            backend: SyslogBackendBuilder::Default,
            format: SyslogFormatterKind::Rfc3164,
            emit_hostname: false,
            append_report_ts: false,
        }
    }

    pub fn set_ident(&mut self, ident: String) {
        self.ident = ident;
    }

    pub fn set_facility(&mut self, facility: Facility) {
        self.facility = facility;
    }

    pub fn set_backend(&mut self, kind: SyslogBackendBuilder) {
        self.backend = kind;
    }

    pub fn set_format(&mut self, kind: SyslogFormatterKind) {
        self.format = kind;
    }

    pub fn enable_cee_log_syntax(&mut self, event_flag: Option<String>) {
        let event_flag = event_flag.unwrap_or_else(|| crate::format::CEE_EVENT_FLAG.to_string());
        self.format = match &self.format {
            SyslogFormatterKind::Rfc3164 | SyslogFormatterKind::Rfc3164Cee(_) => {
                SyslogFormatterKind::Rfc3164Cee(event_flag)
            }
            SyslogFormatterKind::Rfc5424(_, mid) | SyslogFormatterKind::Rfc5424Cee(mid, _) => {
                SyslogFormatterKind::Rfc5424Cee(mid.clone(), event_flag)
            }
        };
    }

    pub fn set_emit_hostname(&mut self, enable: bool) {
        self.emit_hostname = enable;
    }

    pub fn append_report_ts(&mut self, enable: bool) {
        self.append_report_ts = enable;
    }

    pub fn start_async(self, async_conf: &AsyncLogConfig) -> AsyncSyslogStreamer {
        let hostname = if self.emit_hostname {
            if let Ok(r) = nix::unistd::gethostname() {
                r.into_string().ok()
            } else {
                None
            }
        } else {
            None
        };

        let header = SyslogHeader {
            facility: self.facility,
            hostname,
            process: self.ident,
            pid: std::process::id(),
        };

        let mut formatter = match self.format {
            SyslogFormatterKind::Rfc3164 => {
                let formatter = format::FormatterRfc3164::new();
                Box::new(formatter) as BoxSyslogFormatter
            }
            SyslogFormatterKind::Rfc3164Cee(event_flag) => {
                let formatter = format::FormatterRfc3164Cee::new(event_flag);
                Box::new(formatter) as BoxSyslogFormatter
            }
            SyslogFormatterKind::Rfc5424(eid, mid) => {
                let formatter = format::FormatterRfc5424::new(eid, mid);
                Box::new(formatter) as BoxSyslogFormatter
            }
            SyslogFormatterKind::Rfc5424Cee(mid, event_flag) => {
                let formatter = format::FormatterRfc5424Cee::new(mid, event_flag);
                Box::new(formatter) as BoxSyslogFormatter
            }
        };
        formatter.append_report_ts(self.append_report_ts);
        AsyncSyslogStreamer::new(async_conf, header, formatter, &self.backend)
    }
}
