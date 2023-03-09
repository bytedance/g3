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

use std::cell::RefCell;
use std::fmt::{Arguments, Write};
use std::io;

use chrono::{DateTime, Local};
use itoa::Integer;
use ryu::Float;
use slog::{Level, OwnedKVList, Record, Serializer, KV};

use super::{SyslogFormatter, SyslogHeader};
use crate::util::{encode_priority, level_to_severity};

thread_local! {
    static TL_BUF: RefCell<String> = RefCell::new(String::with_capacity(128));
    static TL_VBUF: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(128));
}

pub(crate) struct FormatterRfc3164 {
    append_report_ts: bool,
}

impl FormatterRfc3164 {
    pub(crate) fn new() -> Self {
        FormatterRfc3164 {
            append_report_ts: false,
        }
    }
}

impl SyslogFormatter for FormatterRfc3164 {
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

        let report_ts = if self.append_report_ts {
            Some(datetime_now.timestamp())
        } else {
            None
        };
        format_content_as_text(w, record, logger_values, report_ts)
    }
}

pub(super) fn format_rfc3164_header(
    w: &mut Vec<u8>,
    header: &SyslogHeader,
    level: Level,
    datetime_now: &DateTime<Local>,
) -> io::Result<()> {
    use std::io::Write;

    let priority = encode_priority(level_to_severity(level), header.facility);
    let datetime_fmt = datetime_now.format_with_items(g3_datetime::format::log::RFC3164.iter());

    let mut buffer = itoa::Buffer::new();

    w.push(b'<');
    let priority_s = buffer.format(priority);
    w.extend_from_slice(priority_s.as_bytes());
    w.push(b'>');
    write!(w, "{datetime_fmt}")?;

    if let Some(hostname) = &header.hostname {
        w.push(b' ');
        w.extend_from_slice(hostname.as_bytes());
    }

    w.push(b' ');
    w.extend_from_slice(header.process.as_bytes());
    w.push(b'[');
    let pid_s = buffer.format(header.pid);
    w.extend_from_slice(pid_s.as_bytes());
    w.extend_from_slice(b"]: ");

    Ok(())
}

fn format_content_as_text(
    w: &mut Vec<u8>,
    record: &Record,
    logger_values: &OwnedKVList,
    report_ts: Option<i64>,
) -> Result<(), slog::Error> {
    let mut kv_formatter = FormatterKv(w);
    let msg = record.msg().to_string();
    kv_formatter.push_str_value(&msg);
    logger_values.serialize(record, &mut kv_formatter)?;
    record.kv().serialize(record, &mut kv_formatter)?;
    if let Some(ts) = report_ts {
        kv_formatter.append_report_ts(ts);
    }
    Ok(())
}

struct FormatterKv<'a>(&'a mut Vec<u8>);

impl<'a> FormatterKv<'a> {
    fn push_before_value(&mut self, key: &str) {
        self.0.reserve(key.len() + 2);
        self.0.extend_from_slice(b", ");
        self.0.extend_from_slice(key.as_bytes());
    }

    #[inline]
    fn push_delimiter(&mut self) {
        self.0.push(b'=');
    }

    #[inline]
    fn push_quote(&mut self) {
        self.0.push(b'\"');
    }

    fn push_char_value(&mut self, c: char) {
        for e in c.escape_debug() {
            // same as String.push()
            match e.len_utf8() {
                1 => self.0.push(e as u8),
                _ => self
                    .0
                    .extend_from_slice(e.encode_utf8(&mut [0u8; 4]).as_bytes()),
            }
        }
    }

    fn push_str_value(&mut self, v: &str) {
        self.0.reserve(v.len() << 1); // reserve double space
        v.chars().for_each(|c| self.push_char_value(c));
    }

    fn append_report_ts(&mut self, timestamp: i64) {
        self.0.extend_from_slice(b", report_ts=");

        let mut buffer = itoa::Buffer::new();
        let timestamp_s = buffer.format(timestamp);
        self.0.extend_from_slice(timestamp_s.as_bytes());
    }

    fn emit_integer<T: Integer>(&mut self, key: &str, value: T) -> slog::Result {
        self.push_before_value(key);
        self.push_delimiter();

        let mut buffer = itoa::Buffer::new();
        let value_s = buffer.format(value);
        self.0.extend_from_slice(value_s.as_bytes());
        Ok(())
    }

    fn emit_float<T: Float>(&mut self, key: &str, value: T) -> slog::Result {
        self.push_before_value(key);
        self.push_delimiter();

        let mut buffer = ryu::Buffer::new();
        let value_s = buffer.format(value);
        self.0.extend_from_slice(value_s.as_bytes());
        Ok(())
    }
}

impl<'a> Serializer for FormatterKv<'a> {
    impl_integer_by_itoa! {
        /// Emit `usize`
        usize => emit_usize
    }
    impl_integer_by_itoa! {
        /// Emit `isize`
        isize => emit_isize
    }
    impl_integer_by_itoa! {
        /// Emit `u8`
        u8 => emit_u8
    }
    impl_integer_by_itoa! {
        /// Emit `i8`
        i8 => emit_i8
    }
    impl_integer_by_itoa! {
        /// Emit `u16`
        u16 => emit_u16
    }
    impl_integer_by_itoa! {
        /// Emit `i16`
        i16 => emit_i16
    }
    impl_integer_by_itoa! {
        /// Emit `u32`
        u32 => emit_u32
    }
    impl_integer_by_itoa! {
        /// Emit `i32`
        i32 => emit_i32
    }
    impl_float_by_ryu! {
        /// Emit `f32`
        f32 => emit_f32
    }
    impl_integer_by_itoa! {
        /// Emit `u64`
        u64 => emit_u64
    }
    impl_integer_by_itoa! {
        /// Emit `i64`
        i64 => emit_i64
    }
    impl_float_by_ryu! {
        /// Emit `f64`
        f64 => emit_f64
    }

    fn emit_bool(&mut self, key: slog::Key, value: bool) -> slog::Result {
        self.push_before_value(key);

        if value {
            self.0.extend_from_slice(b"=true");
        } else {
            self.0.extend_from_slice(b"=false");
        }
        Ok(())
    }

    fn emit_char(&mut self, key: slog::Key, value: char) -> slog::Result {
        self.push_before_value(key);
        self.push_delimiter();

        self.push_quote();
        self.push_char_value(value);
        self.push_quote();
        Ok(())
    }

    fn emit_none(&mut self, _key: slog::Key) -> slog::Result {
        Ok(())
    }

    fn emit_str(&mut self, key: slog::Key, value: &str) -> slog::Result {
        self.push_before_value(key);
        self.push_delimiter();

        self.push_quote();
        self.push_str_value(value);
        self.push_quote();
        Ok(())
    }

    impl_arguments_with_tls! {}
    impl_serde_with_tls! {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Facility;
    use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime};

    #[test]
    fn format_header() {
        let lh = SyslogHeader {
            facility: Facility::Daemon,
            hostname: None,
            process: "test".to_string(),
            pid: 1024,
        };

        let mut buffer: Vec<u8> = Vec::new();
        let datetime = NaiveDateTime::new(
            NaiveDate::from_ymd_opt(2021, 12, 1).unwrap(),
            NaiveTime::from_hms_opt(10, 20, 30).unwrap(),
        );
        let dt = DateTime::from_local(datetime, FixedOffset::east_opt(8).unwrap());
        format_rfc3164_header(&mut buffer, &lh, Level::Info, &dt).unwrap();

        let s = String::from_utf8(buffer).unwrap();
        assert_eq!(s, "<29>Dec  1 10:20:30 test[1024]: ");
    }

    #[test]
    fn escape_ascii() {
        let origin = "a\"d\"\ta";
        let mut buf: Vec<u8> = Vec::with_capacity(origin.len());
        let mut f = FormatterKv(&mut buf);
        f.push_str_value(origin);
        let escaped = std::str::from_utf8(&buf).unwrap();
        assert_eq!("a\\\"d\\\"\\ta", escaped);
    }

    #[test]
    fn escape_utf8() {
        let origin = "时间\t\"Time\"";
        let mut buf: Vec<u8> = Vec::with_capacity(origin.len());
        let mut f = FormatterKv(&mut buf);
        f.push_str_value(origin);
        let escaped = std::str::from_utf8(&buf).unwrap();
        assert_eq!("时间\\t\\\"Time\\\"", escaped);
    }

    #[test]
    fn format_u8() {
        let mut vec = Vec::new();
        let mut kv_formatter = FormatterKv(&mut vec);
        kv_formatter.emit_u8("a-key", 8u8).unwrap();
        assert_eq!(std::str::from_utf8(&vec).unwrap(), ", a-key=8");
    }

    #[test]
    fn format_f32() {
        let mut vec = Vec::new();
        let mut kv_formatter = FormatterKv(&mut vec);
        kv_formatter.emit_f32("a-key", 1.1f32).unwrap();
        assert_eq!(std::str::from_utf8(&vec).unwrap(), ", a-key=1.1");
    }

    #[test]
    fn format_bool() {
        let mut vec = Vec::new();
        let mut kv_formatter = FormatterKv(&mut vec);
        kv_formatter.emit_bool("a-key", true).unwrap();
        assert_eq!(std::str::from_utf8(&vec).unwrap(), ", a-key=true");
    }

    #[test]
    fn format_argument() {
        let mut vec = Vec::new();
        let mut kv_formatter = FormatterKv(&mut vec);
        let v = "value";
        kv_formatter
            .emit_arguments("a-key", &format_args!("a-{v}"))
            .unwrap();
        assert_eq!(std::str::from_utf8(&vec).unwrap(), ", a-key=\"a-value\"");
    }
}
