/*
 * Copyright 2025 ByteDance and/or its affiliates.
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
