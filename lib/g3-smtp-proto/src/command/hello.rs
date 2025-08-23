/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use g3_types::net::Host;

use super::CommandLineError;

pub(super) fn parse_host(msg: &[u8]) -> Result<Host, CommandLineError> {
    let host_b = match memchr::memchr(b' ', msg) {
        Some(p) => &msg[..p],
        None => msg,
    };
    Host::parse_smtp_host_address(host_b).ok_or(CommandLineError::InvalidClientHost)
}
