/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use g3_types::net::{Host, UpstreamAddr};

use super::UriParseError;

pub enum HttpMasque {
    Udp(UpstreamAddr),
    Ip(Option<Host>, Option<u16>),
}

impl HttpMasque {
    pub(super) fn new_udp(host: &str, port: &str) -> Result<Self, UriParseError> {
        let host = Host::from_str(host).map_err(|_| UriParseError::NotValidHost("target_host"))?;
        let port = u16::from_str(port).map_err(|_| UriParseError::NotValidPort("target_port"))?;
        Ok(HttpMasque::Udp(UpstreamAddr::new(host, port)))
    }

    pub(super) fn new_ip(host: &str, proto: &str) -> Result<Self, UriParseError> {
        let host = if host.eq("*") {
            None
        } else {
            Some(Host::from_str(host).map_err(|_| UriParseError::NotValidHost("target"))?)
        };
        let proto = if proto.eq("*") {
            None
        } else {
            Some(u16::from_str(proto).map_err(|_| UriParseError::NotValidProtocol("ipproto"))?)
        };
        Ok(HttpMasque::Ip(host, proto))
    }
}
