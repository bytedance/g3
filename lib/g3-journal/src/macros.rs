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
