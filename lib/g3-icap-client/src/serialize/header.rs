/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io::Write;
use std::net::SocketAddr;

use base64::prelude::*;
use bytes::BufMut;

use g3_types::net::HttpHeaderMap;

pub(crate) fn add_client_addr(buf: &mut Vec<u8>, addr: SocketAddr) {
    let _ = write!(buf, "X-Client-IP: {}\r\n", addr.ip());
    let _ = write!(buf, "X-Client-Port: {}\r\n", addr.port());
}

pub(crate) fn add_client_username(buf: &mut Vec<u8>, user: &str) {
    buf.put_slice(b"X-Client-Username: ");
    buf.put_slice(user.as_bytes());
    buf.put_slice(b"\r\n");

    buf.put_slice(b"X-Authenticated-User: ");
    let v = BASE64_STANDARD.encode(format!("Local://{user}"));
    buf.put_slice(v.as_bytes());
    buf.put_slice(b"\r\n");
}

pub(crate) fn add_shared(buf: &mut Vec<u8>, headers: &HttpHeaderMap) {
    headers.for_each(|name, value| {
        buf.put_slice(name.as_str().as_bytes());
        buf.put_slice(b": ");
        buf.put_slice(value.as_bytes());
        buf.put_slice(b"\r\n");
    });
}
