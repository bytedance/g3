/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

#[macro_export]
macro_rules! impl_encode {
    ($(#[$m:meta])* $t:ty => $f:ident, $w:ident) => {
        $(#[$m])*
        fn $f(&mut self, key : slog::Key, val : $t)
            -> slog::Result {
                self.write_key(key.as_str())?;
                rmp::encode::$w(&mut self.0, val).map_err(|e| slog::Error::Io(e.into()))
            }
    };
}
