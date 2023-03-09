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

#[macro_export]
macro_rules! impl_integer_by_itoa {
    ($(#[$m:meta])* $t:ty => $f:ident) => {
        $(#[$m])*
        fn $f(&mut self, key : slog::Key, val : $t)
            -> slog::Result {
                self.emit_integer(key, val)
            }
    };
}

#[macro_export]
macro_rules! impl_float_by_ryu {
    ($(#[$m:meta])* $t:ty => $f:ident) => {
        $(#[$m])*
        fn $f(&mut self, key : slog::Key, val : $t)
            -> slog::Result {
                self.emit_float(key, val)
            }
    };
}

#[macro_export]
macro_rules! impl_arguments_with_tls {
    () => {
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
    };
}

#[macro_export]
macro_rules! impl_serde_with_tls {
    () => {
        fn emit_serde(&mut self, key: slog::Key, value: &dyn slog::SerdeValue) -> slog::Result {
            use serde::ser::Serialize;
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
    };
}
