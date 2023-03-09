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

use chrono::{DateTime, Timelike, Utc};
use itoa::Integer;
use ryu::Float;
use slog::{Level, OwnedKVList, Record, Serializer, KV};

use super::{SyslogFormatter, SyslogHeader};
use crate::util::{encode_priority, level_to_severity};

thread_local! {
    static TL_BUF: RefCell<String> = RefCell::new(String::with_capacity(128));
    static TL_VBUF: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(128));
}

pub(crate) struct FormatterRfc5424 {
    enterprise_id: i32,
    message_id: Option<String>,
    append_report_ts: bool,
}

impl FormatterRfc5424 {
    pub(crate) fn new(enterprise_id: i32, message_id: Option<String>) -> Self {
        FormatterRfc5424 {
            enterprise_id,
            message_id,
            append_report_ts: false,
        }
    }
}

impl SyslogFormatter for FormatterRfc5424 {
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

        let report_ts = if self.append_report_ts {
            Some(datetime_now.timestamp())
        } else {
            None
        };
        format_content_as_sd(w, self.enterprise_id, record, logger_values, report_ts)
    }
}

pub(super) fn format_rfc5424_header(
    w: &mut Vec<u8>,
    header: &SyslogHeader,
    level: Level,
    datetime_now: &DateTime<Utc>,
    message_id: &Option<String>,
) -> io::Result<()> {
    use std::io::Write;

    let priority = encode_priority(level_to_severity(level), header.facility);
    let datetime_fmt = if datetime_now.nanosecond() >= 1_000_000_000 {
        let dt = datetime_now.with_nanosecond(999_999_999).unwrap();
        dt.format_with_items(g3_datetime::format::log::RFC5424.iter())
    } else {
        datetime_now.format_with_items(g3_datetime::format::log::RFC5424.iter())
    };

    let mut buffer = itoa::Buffer::new();

    // write header
    w.push(b'<');
    let priority_s = buffer.format(priority);
    w.extend_from_slice(priority_s.as_bytes());
    w.extend_from_slice(b">1 ");
    write!(w, "{datetime_fmt}")?;
    w.push(b' ');
    match &header.hostname {
        Some(hostname) => w.extend_from_slice(hostname.as_bytes()),
        None => w.push(b'-'),
    };
    w.push(b' ');
    w.extend_from_slice(header.process.as_bytes());
    w.push(b' ');
    let pid_s = buffer.format(header.pid);
    w.extend_from_slice(pid_s.as_bytes());
    w.push(b' ');
    match &message_id {
        Some(id) => w.extend_from_slice(id.as_bytes()),
        None => w.push(b'-'),
    };
    w.push(b' ');

    Ok(())
}

fn format_content_as_sd(
    w: &mut Vec<u8>,
    enterprise_id: i32,
    record: &Record,
    logger_values: &OwnedKVList,
    report_ts: Option<i64>,
) -> Result<(), slog::Error> {
    w.extend_from_slice(b"[g3proxy@");
    let mut buffer = itoa::Buffer::new();
    let eid_s = buffer.format(enterprise_id);
    w.extend_from_slice(eid_s.as_bytes());

    let mut kv_formatter = FormatterKv(w);
    logger_values.serialize(record, &mut kv_formatter)?;
    record.kv().serialize(record, &mut kv_formatter)?;
    if let Some(ts) = report_ts {
        kv_formatter.append_report_ts(ts);
    }
    w.push(b']');

    // write msg
    w.push(b' ');
    let msg = record.msg().to_string();
    let mut f = FormatterKv(w);
    f.push_str_value(&msg);

    Ok(())
}

struct FormatterKv<'a>(&'a mut Vec<u8>);

impl<'a> FormatterKv<'a> {
    fn push_before_value(&mut self, key: &str) {
        self.0.reserve(key.len() + 3);
        self.0.push(b' ');
        self.0.extend_from_slice(key.as_bytes());
        self.0.extend_from_slice(b"=\"");
    }

    #[inline]
    fn push_after_value(&mut self) {
        self.0.push(b'\"');
    }

    fn push_char_value(&mut self, c: char) {
        if c == ']' {
            self.0.extend_from_slice(b"\\]");
        } else {
            // '"' and '\' is escaped in escape_debug()
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
    }

    fn push_str_value(&mut self, v: &str) {
        self.0.reserve(v.len() << 1); // reserve double space
        v.chars().for_each(|c| self.push_char_value(c));
    }

    fn append_report_ts(&mut self, timestamp: i64) {
        self.0.extend_from_slice(b" report_ts=\"");

        let mut buffer = itoa::Buffer::new();
        let timestamp_s = buffer.format(timestamp);
        self.0.extend_from_slice(timestamp_s.as_bytes());

        self.push_after_value();
    }

    fn emit_integer<T: Integer>(&mut self, key: &str, value: T) -> slog::Result {
        self.push_before_value(key);

        let mut buffer = itoa::Buffer::new();
        let value_s = buffer.format(value);
        self.0.extend_from_slice(value_s.as_bytes());

        self.push_after_value();
        Ok(())
    }

    fn emit_float<T: Float>(&mut self, key: &str, value: T) -> slog::Result {
        self.push_before_value(key);

        let mut buffer = ryu::Buffer::new();
        let value_s = buffer.format(value);
        self.0.extend_from_slice(value_s.as_bytes());

        self.push_after_value();
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
        self.0.reserve(key.len() + 3);
        self.0.push(b' ');
        self.0.extend_from_slice(key.as_bytes());

        if value {
            self.0.extend_from_slice(b"=\"true\"");
        } else {
            self.0.extend_from_slice(b"=\"false\"");
        }
        Ok(())
    }

    fn emit_char(&mut self, key: slog::Key, value: char) -> slog::Result {
        self.push_before_value(key);

        self.push_char_value(value);

        self.push_after_value();
        Ok(())
    }

    fn emit_none(&mut self, _key: slog::Key) -> slog::Result {
        Ok(())
    }

    fn emit_str(&mut self, key: slog::Key, value: &str) -> slog::Result {
        self.push_before_value(key);

        self.push_str_value(value);

        self.push_after_value();
        Ok(())
    }

    impl_arguments_with_tls! {}
    impl_serde_with_tls! {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Facility;
    use chrono::{DateTime, Utc};

    #[test]
    fn format_header() {
        fn format_header_to_string(lh: &SyslogHeader, datetime: &DateTime<Utc>) -> String {
            let mut buffer: Vec<u8> = Vec::new();
            format_rfc5424_header(&mut buffer, lh, Level::Info, datetime, &None).unwrap();

            String::from_utf8(buffer).unwrap()
        }

        let lh = SyslogHeader {
            facility: Facility::Daemon,
            hostname: None,
            process: "test".to_string(),
            pid: 1024,
        };

        let datetime = DateTime::parse_from_rfc3339("2021-12-01T10:20:30.123456789Z").unwrap();
        let dt = datetime.with_timezone(&Utc);
        assert_eq!(
            format_header_to_string(&lh, &dt),
            "<29>1 2021-12-01T10:20:30.123456Z - test 1024 - "
        );

        let datetime = DateTime::parse_from_rfc3339("2021-12-01T10:20:30.12345Z").unwrap();
        let dt = datetime.with_timezone(&Utc);
        assert_eq!(
            format_header_to_string(&lh, &dt),
            "<29>1 2021-12-01T10:20:30.123450Z - test 1024 - "
        );

        let datetime = DateTime::parse_from_rfc3339("2021-12-01T10:20:30+08:00").unwrap();
        let dt = datetime.with_timezone(&Utc);
        assert_eq!(
            format_header_to_string(&lh, &dt),
            "<29>1 2021-12-01T02:20:30.000000Z - test 1024 - "
        );
    }

    #[test]
    fn escape_ascii() {
        let origin = "[::ffff:1.2.3.4]:8123";
        let mut buf: Vec<u8> = Vec::with_capacity(origin.len());
        let mut f = FormatterKv(&mut buf);
        f.push_str_value(origin);
        let escaped = std::str::from_utf8(&buf).unwrap();
        assert_eq!("[::ffff:1.2.3.4\\]:8123", escaped);
    }

    #[test]
    fn escape_utf8() {
        let origin = "时间[Time]";
        let mut buf: Vec<u8> = Vec::with_capacity(origin.len());
        let mut f = FormatterKv(&mut buf);
        f.push_str_value(origin);
        let escaped = std::str::from_utf8(&buf).unwrap();
        assert_eq!("时间[Time\\]", escaped);
    }

    #[test]
    fn format_u8() {
        let mut vec = Vec::new();
        let mut kv_formatter = FormatterKv(&mut vec);
        kv_formatter.emit_u8("a-key", 8u8).unwrap();
        assert_eq!(std::str::from_utf8(&vec).unwrap(), " a-key=\"8\"");
    }

    #[test]
    fn format_f32() {
        let mut vec = Vec::new();
        let mut kv_formatter = FormatterKv(&mut vec);
        kv_formatter.emit_f32("a-key", 1.1f32).unwrap();
        assert_eq!(std::str::from_utf8(&vec).unwrap(), " a-key=\"1.1\"");
    }

    #[test]
    fn format_bool() {
        let mut vec = Vec::new();
        let mut kv_formatter = FormatterKv(&mut vec);
        kv_formatter.emit_bool("a-key", true).unwrap();
        assert_eq!(std::str::from_utf8(&vec).unwrap(), " a-key=\"true\"");
    }

    #[test]
    fn format_argument() {
        let mut vec = Vec::new();
        let mut kv_formatter = FormatterKv(&mut vec);
        let v = "value";
        kv_formatter
            .emit_arguments("a-key", &format_args!("a-{v}"))
            .unwrap();
        assert_eq!(std::str::from_utf8(&vec).unwrap(), " a-key=\"a-value\"");
    }
}
