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

use chrono::Utc;
use serde::ser::Serialize;
use slog::{OwnedKVList, Record, Serializer, KV};

use g3_types::log::AsyncLogFormatter;

thread_local! {
    static TL_BUF: RefCell<String> = RefCell::new(String::with_capacity(128))
}

struct EncodeError(slog::Error);

impl From<rmp::encode::ValueWriteError> for EncodeError {
    fn from(e: rmp::encode::ValueWriteError) -> Self {
        EncodeError(slog::Error::Io(e.into()))
    }
}

impl From<slog::Error> for EncodeError {
    fn from(e: slog::Error) -> Self {
        EncodeError(e)
    }
}

pub struct FluentdFormatter {
    tag_name: String,
}

impl FluentdFormatter {
    pub(super) fn new(tag_name: String) -> Self {
        FluentdFormatter { tag_name }
    }

    fn rmp_encode(
        &self,
        record: &Record,
        logger_values: &OwnedKVList,
    ) -> Result<Vec<u8>, EncodeError> {
        let datetime_now = Utc::now();
        let mut buf = Vec::<u8>::with_capacity(1024);

        rmp::encode::write_array_len(&mut buf, 3)?;
        {
            // #1
            rmp::encode::write_str(&mut buf, &self.tag_name)?;

            // #2
            rmp::encode::write_ext_meta(&mut buf, 8, 0)?;
            let sec = u32::try_from(datetime_now.timestamp())
                .map_err(|_| {
                    slog::Error::Io(io::Error::new(
                        io::ErrorKind::Other,
                        "out of range unix timestamp",
                    ))
                })?
                .to_be_bytes();
            buf.extend_from_slice(&sec);
            let nano = datetime_now
                .timestamp_subsec_nanos()
                .min(999_999_999) // ignore leap second
                .to_be_bytes();
            buf.extend_from_slice(&nano);

            // #3
            let mut counter = CounterKV(0);
            logger_values.serialize(record, &mut counter)?;
            record.kv().serialize(record, &mut counter)?;
            rmp::encode::write_map_len(&mut buf, counter.0 + 1)?;
            {
                let mut kv_formatter = FormatterKv(&mut buf);
                logger_values.serialize(record, &mut kv_formatter)?;
                record.kv().serialize(record, &mut kv_formatter)?;
                kv_formatter.emit_arguments("msg", record.msg())?;
            }
        }

        Ok(buf)
    }
}

impl AsyncLogFormatter<Vec<u8>> for FluentdFormatter {
    fn format_slog(
        &self,
        record: &Record,
        logger_values: &OwnedKVList,
    ) -> Result<Vec<u8>, slog::Error> {
        let buf = self.rmp_encode(record, logger_values).map_err(|e| e.0)?;
        Ok(buf)
    }
}

struct CounterKV(u32);

impl Serializer for CounterKV {
    fn emit_arguments(&mut self, _key: slog::Key, _val: &Arguments) -> slog::Result {
        self.0 += 1;
        Ok(())
    }
}

struct FormatterKv<'a>(&'a mut Vec<u8>);

impl<'a> FormatterKv<'a> {
    fn write_key(&mut self, key: slog::Key) -> slog::Result {
        rmp::encode::write_str(&mut self.0, key).map_err(|e| slog::Error::Io(e.into()))
    }
}

impl<'a> Serializer for FormatterKv<'a> {
    fn emit_usize(&mut self, key: slog::Key, value: usize) -> slog::Result {
        self.emit_u64(key, value as u64)
    }
    fn emit_isize(&mut self, key: slog::Key, value: isize) -> slog::Result {
        self.emit_i64(key, value as i64)
    }

    impl_encode! {
        u8 => emit_u8, write_u8
    }
    impl_encode! {
        i8 => emit_i8, write_i8
    }
    impl_encode! {
        u16 => emit_u16, write_u16
    }
    impl_encode! {
        i16 => emit_i16, write_i16
    }
    impl_encode! {
        u32 => emit_u32, write_u32
    }
    impl_encode! {
        i32 => emit_i32, write_i32
    }
    impl_encode! {
        u64 => emit_u64, write_u64
    }
    impl_encode! {
        i64 => emit_i64, write_i64
    }

    impl_encode! {
        f32 => emit_f32, write_f32
    }
    impl_encode! {
        f64 => emit_f64, write_f64
    }

    impl_encode! {
        bool => emit_bool, write_bool
    }

    fn emit_char(&mut self, key: slog::Key, value: char) -> slog::Result {
        self.emit_str(key, value.encode_utf8(&mut [0u8, 4]))
    }

    fn emit_none(&mut self, key: slog::Key) -> slog::Result {
        self.write_key(key)?;
        rmp::encode::write_nil(&mut self.0).map_err(slog::Error::Io)
    }

    fn emit_str(&mut self, key: slog::Key, value: &str) -> slog::Result {
        self.write_key(key)?;
        rmp::encode::write_str(&mut self.0, value).map_err(|e| slog::Error::Io(e.into()))
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
        self.write_key(key)?;
        let mut serializer = rmp_serde::Serializer::new(&mut self.0);
        value.as_serde().serialize(&mut serializer).map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("serde serialization error for key {key}: {e}"),
            )
        })?;
        Ok(())
    }
}
