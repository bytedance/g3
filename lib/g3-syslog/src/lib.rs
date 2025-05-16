/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_types::log::AsyncLogConfig;

mod async_streamer;

mod backend;
mod format;
mod types;
mod util;

#[cfg(feature = "yaml")]
mod yaml;

pub use types::{Facility, Severity};

use async_streamer::AsyncSyslogStreamer;

pub use backend::SyslogBackendBuilder;

use format::BoxSyslogFormatter;
pub use format::SyslogFormatterKind;

pub struct SyslogHeader {
    pub facility: Facility,
    pub hostname: Option<String>,
    pub process: &'static str,
    pub pid: u32,
}

#[derive(Clone, Debug)]
pub struct SyslogBuilder {
    ident: &'static str,
    facility: Facility,
    backend: SyslogBackendBuilder,
    format: SyslogFormatterKind,
    emit_hostname: bool,
    append_report_ts: bool,
}

impl SyslogBuilder {
    pub fn with_ident(ident: &'static str) -> Self {
        SyslogBuilder {
            ident,
            facility: Facility::User,
            backend: SyslogBackendBuilder::default(),
            format: SyslogFormatterKind::Rfc3164,
            emit_hostname: false,
            append_report_ts: false,
        }
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
            Some(g3_compat::hostname().to_string_lossy().to_string())
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
