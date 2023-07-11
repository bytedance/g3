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

use std::net::IpAddr;

use slog::{Record, Serializer, Value};

use g3_types::net::UpstreamAddr;

pub struct LtIpAddr(pub IpAddr);

impl Value for LtIpAddr {
    fn serialize(
        &self,
        _record: &Record,
        key: slog::Key,
        serializer: &mut dyn Serializer,
    ) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self.0))
    }
}

pub struct LtUpstreamAddr<'a>(pub &'a UpstreamAddr);

impl<'a> Value for LtUpstreamAddr<'a> {
    fn serialize(
        &self,
        _record: &Record,
        key: slog::Key,
        serializer: &mut dyn Serializer,
    ) -> slog::Result {
        if self.0.is_empty() {
            serializer.emit_none(key)
        } else {
            serializer.emit_arguments(key, &format_args!("{}", &self.0))
        }
    }
}
