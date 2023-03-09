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

use std::io;

use chrono::{Local, Utc};
use slog::{OwnedKVList, Record, Serializer, KV};

use super::rfc3164::format_rfc3164_header;
use super::rfc5424::format_rfc5424_header;
use super::serde::SerdeFormatterKV;
use super::{SyslogFormatter, SyslogHeader};

pub(crate) const CEE_EVENT_FLAG: &str = "@cee:";

pub(crate) struct FormatterRfc3164Cee {
    event_flag: String,
    append_report_ts: bool,
}

impl FormatterRfc3164Cee {
    pub(crate) fn new(event_flag: String) -> Self {
        FormatterRfc3164Cee {
            event_flag,
            append_report_ts: false,
        }
    }
}

impl SyslogFormatter for FormatterRfc3164Cee {
    fn append_report_ts(&mut self, enable: bool) {
        self.append_report_ts = enable;
    }

    fn format_slog(
        &self,
        w: &mut Vec<u8>,
        header: &SyslogHeader,
        record: &Record,
        logger_values: &OwnedKVList,
    ) -> Result<(), slog::Error> {
        let datetime_now = Local::now();

        format_rfc3164_header(w, header, record.level(), &datetime_now)?;

        w.extend_from_slice(self.event_flag.as_bytes());

        let report_ts = if self.append_report_ts {
            Some(datetime_now.timestamp())
        } else {
            None
        };
        format_content_as_json(w, record, logger_values, report_ts)
    }
}

pub(crate) struct FormatterRfc5424Cee {
    message_id: Option<String>,
    event_flag: String,
    append_report_ts: bool,
}

impl FormatterRfc5424Cee {
    pub(crate) fn new(message_id: Option<String>, event_flag: String) -> Self {
        FormatterRfc5424Cee {
            message_id,
            event_flag,
            append_report_ts: false,
        }
    }
}

impl SyslogFormatter for FormatterRfc5424Cee {
    fn append_report_ts(&mut self, enable: bool) {
        self.append_report_ts = enable;
    }

    fn format_slog(
        &self,
        w: &mut Vec<u8>,
        header: &SyslogHeader,
        record: &Record,
        logger_values: &OwnedKVList,
    ) -> Result<(), slog::Error> {
        let datetime_now = Utc::now();

        format_rfc5424_header(w, header, record.level(), &datetime_now, &self.message_id)?;

        w.extend_from_slice(self.event_flag.as_bytes());

        let report_ts = if self.append_report_ts {
            Some(datetime_now.timestamp())
        } else {
            None
        };
        format_content_as_json(w, record, logger_values, report_ts)
    }
}

fn format_content_as_json(
    w: &mut Vec<u8>,
    record: &Record,
    logger_values: &OwnedKVList,
    report_ts: Option<i64>,
) -> Result<(), slog::Error> {
    let mut serde = serde_json::Serializer::new(w);

    let mut kv_formatter = SerdeFormatterKV::start(&mut serde, None)?;
    logger_values.serialize(record, &mut kv_formatter)?;
    record.kv().serialize(record, &mut kv_formatter)?;

    if let Some(ts) = report_ts {
        kv_formatter.emit_i64("report_ts", ts)?;
    }

    kv_formatter.emit_arguments("msg", record.msg())?;

    kv_formatter
        .end()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    Ok(())
}
