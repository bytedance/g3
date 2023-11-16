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

use itoa::Integer;
use ryu::Float;
use slog::{Error, Level, OwnedKVList, Record, Serializer, KV};

use g3_types::log::AsyncLogFormatter;

use super::JournalConfig;

thread_local! {
    static TL_BUF: RefCell<String> = RefCell::new(String::with_capacity(128));
    static TL_VBUF: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(128));
}

fn level_to_sd_priority(level: Level) -> &'static str {
    match level {
        Level::Critical => "2", // LOG_CRIT
        Level::Error => "3",    // LOG_ERR
        Level::Warning => "4",  // LOG_WARNING
        Level::Info => "5",     // LOG_NOTICE
        Level::Debug => "6",    // LOG_INFO
        Level::Trace => "7",    // LOG_DEBUG
    }
}

pub struct JournalFormatter {
    conf: JournalConfig,
}

impl JournalFormatter {
    pub(super) fn new(conf: JournalConfig) -> Self {
        JournalFormatter { conf }
    }
}

impl AsyncLogFormatter<Vec<u8>> for JournalFormatter {
    fn format_slog(&self, record: &Record, logger_values: &OwnedKVList) -> Result<Vec<u8>, Error> {
        let mut buf = Vec::with_capacity(1024);
        let mut kv_formatter = FormatterKv(&mut buf);

        kv_formatter.emit_sanitized_one_line("PRIORITY", level_to_sd_priority(record.level()));
        kv_formatter.emit_sanitized_one_line("SYSLOG_IDENTIFIER", self.conf.ident);

        logger_values.serialize(record, &mut kv_formatter)?;
        record.kv().serialize(record, &mut kv_formatter)?;

        if self.conf.append_code_position {
            let code_position = match record.file().rsplit_once('/').map(|x| x.1) {
                Some(filename) => format!("{}({filename}:{})", record.module(), record.line()),
                None => record.module().to_string(),
            };
            kv_formatter.emit_sanitized_one_line("CODE_POSITION", &code_position);
        }

        kv_formatter.emit_arguments("MESSAGE", record.msg())?;

        Ok(buf)
    }
}

struct FormatterKv<'a>(&'a mut Vec<u8>);

impl<'a> FormatterKv<'a> {
    fn emit_integer<T: Integer>(&mut self, key: slog::Key, value: T) -> slog::Result {
        let mut buffer = itoa::Buffer::new();
        let value_s = buffer.format(value);
        self.emit_one_line(key, value_s)
    }

    fn emit_float<T: Float>(&mut self, key: slog::Key, value: T) -> slog::Result {
        let mut buffer = ryu::Buffer::new();
        let value_s = buffer.format(value);
        self.emit_one_line(key, value_s)
    }

    fn emit_sanitized_one_line(&mut self, key: &str, value: &str) {
        self.0.extend_from_slice(key.as_bytes());
        self.0.push(b'=');
        self.0.extend_from_slice(value.as_bytes());
        self.0.push(b'\n');
    }

    fn emit_one_line(&mut self, key: slog::Key, value: &str) -> slog::Result {
        if let Some(k) = sanitized_key(key) {
            self.emit_sanitized_one_line(&k, value);
        }
        Ok(())
    }

    fn emit_multi_line(&mut self, key: slog::Key, value: &str) -> slog::Result {
        if let Some(k) = sanitized_key(key) {
            self.0.extend_from_slice(k.as_bytes());
            self.0.push(b'\n');
            let len = value.len() as u64;
            self.0.extend_from_slice(&len.to_le_bytes());
            self.0.extend_from_slice(value.as_bytes());
            self.0.push(b'\n');
        }
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
        if value {
            self.emit_one_line(key, "true")
        } else {
            self.emit_one_line(key, "false")
        }
    }

    fn emit_char(&mut self, key: slog::Key, value: char) -> slog::Result {
        if value == '\n' {
            self.emit_multi_line(key, "\n")
        } else {
            self.emit_one_line(key, value.encode_utf8(&mut [0u8; 4]))
        }
    }

    fn emit_none(&mut self, _key: slog::Key) -> slog::Result {
        Ok(())
    }

    fn emit_str(&mut self, key: slog::Key, value: &str) -> slog::Result {
        if memchr::memchr(b'\n', value.as_bytes()).is_some() {
            self.emit_multi_line(key, value)
        } else {
            self.emit_one_line(key, value)
        }
    }

    fn emit_arguments(&mut self, key: slog::Key, value: &Arguments) -> slog::Result {
        if let Some(s) = value.as_str() {
            self.emit_str(key, s)
        } else {
            TL_BUF.with_borrow_mut(|buf| {
                buf.clear();

                buf.write_fmt(*value).unwrap();

                self.emit_str(key, buf.as_str())
            })
        }
    }

    fn emit_serde(&mut self, key: slog::Key, value: &dyn slog::SerdeValue) -> slog::Result {
        use serde::ser::Serialize;
        use std::io;
        use std::ops::DerefMut;

        TL_VBUF.with_borrow_mut(|buf| {
            buf.clear();

            let mut serializer = serde_json::Serializer::new(buf.deref_mut());
            value.as_serde().serialize(&mut serializer).map_err(|e| {
                io::Error::other(format!("serde serialization error for key {key}: {e}"))
            })?;

            let v = std::str::from_utf8(buf)
                .map_err(|e| io::Error::other(format!("invalid utf-8 value for key {key}: {e}")))?;
            self.emit_str(key, v)
        })
    }
}

fn sanitized_key(s: &str) -> Option<String> {
    if s.is_empty() || s.len() > 64 || !s.as_bytes()[0].is_ascii_alphabetic() {
        return None;
    }

    let mut k = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'A'..='Z' | '0'..='9' => k.push(c),
            'a'..='z' => k.push(c.to_ascii_uppercase()),
            _ => k.push('_'),
        }
    }
    Some(k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_key() {
        assert_eq!(sanitized_key("A_KEY"), Some("A_KEY".to_string()));
        assert_eq!(sanitized_key("A-KEY"), Some("A_KEY".to_string()));
        assert_eq!(sanitized_key("a_KeY"), Some("A_KEY".to_string()));

        assert!(sanitized_key("_a").is_none());
        assert!(sanitized_key("1a").is_none());
        assert!(sanitized_key("-a").is_none());
    }

    #[test]
    fn format_u8() {
        let mut vars = Vec::new();
        let mut kv_formatter = FormatterKv(&mut vars);

        kv_formatter.emit_u8("a-key", 8u8).unwrap();
        assert_eq!(vars, b"A_KEY=8\n");
    }

    #[test]
    fn format_f32() {
        let mut vars = Vec::new();
        let mut kv_formatter = FormatterKv(&mut vars);

        kv_formatter.emit_f32("a-key", 1.1f32).unwrap();
        assert_eq!(vars, b"A_KEY=1.1\n");
    }

    #[test]
    fn format_bool() {
        let mut vars = Vec::new();
        let mut kv_formatter = FormatterKv(&mut vars);

        kv_formatter.emit_bool("a-key", true).unwrap();
        assert_eq!(vars, b"A_KEY=true\n");
    }

    #[test]
    fn format_argument() {
        let mut vars = Vec::new();
        let mut kv_formatter = FormatterKv(&mut vars);

        let v = "value";
        kv_formatter
            .emit_arguments("a-key", &format_args!("a-{v}"))
            .unwrap();
        assert_eq!(vars, b"A_KEY=a-value\n");
    }

    #[test]
    fn format_newline() {
        let mut vars = Vec::new();
        let mut kv_formatter = FormatterKv(&mut vars);

        let v = "v1\nv2";
        kv_formatter
            .emit_arguments("a-key", &format_args!("a-{v}"))
            .unwrap();
        assert_eq!(vars, b"A_KEY\n\x07\0\0\0\0\0\0\0a-v1\nv2\n");
    }
}
