/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use slog::{Record, Serializer, Value};

use g3_socket::BindAddr;

use crate::LtIpAddr;

pub struct LtBindAddr(pub BindAddr);

impl Value for LtBindAddr {
    fn serialize(
        &self,
        _record: &Record,
        key: slog::Key,
        serializer: &mut dyn Serializer,
    ) -> slog::Result {
        match self.0 {
            BindAddr::None => serializer.emit_none(key),
            BindAddr::Ip(ip) => LtIpAddr(ip).serialize(_record, key, serializer),
            #[cfg(any(
                target_os = "linux",
                target_os = "android",
                target_os = "macos",
                target_os = "illumos",
                target_os = "solaris"
            ))]
            BindAddr::Interface(name) => serializer.emit_str(key, name.name()),
        }
    }
}
