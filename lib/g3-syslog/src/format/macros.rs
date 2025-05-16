/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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
                TL_BUF.with_borrow_mut(|buf| {
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

            TL_VBUF.with_borrow_mut(|buf| {
                buf.clear();

                let mut serializer = serde_json::Serializer::new(buf.deref_mut());
                value.as_serde().serialize(&mut serializer).map_err(|e| {
                    io::Error::other(format!("serde serialization error for key {key}: {e}"))
                })?;

                let v = std::str::from_utf8(&buf).map_err(|e| {
                    io::Error::other(format!("invalid utf-8 value for key {key}: {e}"))
                })?;
                self.emit_str(key, v)
            })
        }
    };
}
