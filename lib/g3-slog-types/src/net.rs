/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::{IpAddr, SocketAddr};

use slog::{Record, Serializer, Value};

use g3_types::net::{Host, UpstreamAddr};

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

pub struct LtSocketAddr(pub SocketAddr);

impl Value for LtSocketAddr {
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

impl Value for LtUpstreamAddr<'_> {
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

pub struct LtHost<'a>(pub &'a Host);

impl Value for LtHost<'_> {
    fn serialize(
        &self,
        _record: &Record,
        key: slog::Key,
        serializer: &mut dyn Serializer,
    ) -> slog::Result {
        if self.0.is_empty() {
            serializer.emit_none(key)
        } else {
            match self.0 {
                Host::Domain(s) => serializer.emit_str(key, s),
                Host::Ip(ip) => serializer.emit_arguments(key, &format_args!("{ip}")),
            }
        }
    }
}
