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
use libsystemd::logging::Priority;
use ryu::Float;
use slog::{Error, Level, OwnedKVList, Record, Serializer, KV};

use g3_types::log::AsyncLogFormatter;

use super::JournalValue;

thread_local! {
    static TL_BUF: RefCell<String> = RefCell::new(String::with_capacity(128));
    static TL_VBUF: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(128));
}

fn level_to_sd_priority(level: Level) -> Priority {
    match level {
        Level::Critical => Priority::Critical,
        Level::Error => Priority::Error,
        Level::Warning => Priority::Warning,
        Level::Info => Priority::Notice,
        Level::Debug => Priority::Info,
        Level::Trace => Priority::Debug,
    }
}

pub struct JournalFormatter {
    append_code_position: bool,
}

impl JournalFormatter {
    pub(super) fn new(append_code_position: bool) -> Self {
        JournalFormatter {
            append_code_position,
        }
    }
}

impl AsyncLogFormatter<JournalValue> for JournalFormatter {
    fn format_slog(
        &self,
        record: &Record,
        logger_values: &OwnedKVList,
    ) -> Result<JournalValue, Error> {
        let mut vars = Vec::new();
        let mut kv_formatter = FormatterKv(&mut vars);

        logger_values.serialize(record, &mut kv_formatter)?;
        record.kv().serialize(record, &mut kv_formatter)?;

        if self.append_code_position {
            let code_position = match record.file().rsplit_once('/').map(|x| x.1) {
                Some(filename) => format!("{}({filename}:{})", record.module(), record.line()),
                None => record.module().to_string(),
            };
            vars.push(("CODE_POSITION".to_string(), code_position));
        }

        Ok(JournalValue {
            priority: level_to_sd_priority(record.level()),
            msg: record.msg().to_string(),
            vars,
        })
    }
}

struct FormatterKv<'a>(&'a mut Vec<(String, String)>);

impl<'a> FormatterKv<'a> {
    fn emit_integer<T: Integer>(&mut self, key: slog::Key, value: T) -> slog::Result {
        let mut buffer = itoa::Buffer::new();
        let value_s = buffer.format(value);
        self.emit_str(key, value_s)
    }

    fn emit_float<T: Float>(&mut self, key: slog::Key, value: T) -> slog::Result {
        let mut buffer = ryu::Buffer::new();
        let value_s = buffer.format(value);
        self.emit_str(key, value_s)
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
            self.emit_str(key, "true")
        } else {
            self.emit_str(key, "false")
        }
    }

    fn emit_char(&mut self, key: slog::Key, value: char) -> slog::Result {
        self.emit_str(key, value.encode_utf8(&mut [0u8; 4]))
    }

    fn emit_none(&mut self, _key: slog::Key) -> slog::Result {
        Ok(())
    }

    fn emit_str(&mut self, key: slog::Key, value: &str) -> slog::Result {
        if let Some(k) = sanitized_key(key) {
            let v = value.to_string();
            self.0.push((k, v));
        }
        Ok(())
    }

    fn emit_arguments(&mut self, key: slog::Key, value: &Arguments) -> slog::Result {
        if let Some(s) = value.as_str() {
            self.emit_str(key, s)
        } else {
            TL_BUF.with(|buf| {
                let mut buf = buf.borrow_mut();
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

        TL_VBUF.with(|buf| {
            let mut buf = buf.borrow_mut();
            buf.clear();

            let mut serializer = serde_json::Serializer::new(buf.deref_mut());
            value.as_serde().serialize(&mut serializer).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::Other,
                    format!("serde serialization error for key {key}: {e}"),
                )
            })?;

            let v = std::str::from_utf8(&buf).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::Other,
                    format!("invalid utf-8 value for key {key}: {e}"),
                )
            })?;
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
        assert_eq!(vars, [("A_KEY".to_string(), "8".to_string())]);
    }

    #[test]
    fn format_f32() {
        let mut vars = Vec::new();
        let mut kv_formatter = FormatterKv(&mut vars);

        kv_formatter.emit_f32("a-key", 1.1f32).unwrap();
        assert_eq!(vars, [("A_KEY".to_string(), "1.1".to_string())]);
    }

    #[test]
    fn format_bool() {
        let mut vars = Vec::new();
        let mut kv_formatter = FormatterKv(&mut vars);

        kv_formatter.emit_bool("a-key", true).unwrap();
        assert_eq!(vars, [("A_KEY".to_string(), "true".to_string())]);
    }

    #[test]
    fn format_argument() {
        let mut vars = Vec::new();
        let mut kv_formatter = FormatterKv(&mut vars);

        let v = "value";
        kv_formatter
            .emit_arguments("a-key", &format_args!("a-{v}"))
            .unwrap();
        assert_eq!(vars, [("A_KEY".to_string(), "a-value".to_string())]);
    }
}
