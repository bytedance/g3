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
