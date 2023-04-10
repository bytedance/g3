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

use std::cell::RefCell;
use std::io::Write;
use std::net::{IpAddr, SocketAddr};

use base64::prelude::*;
use chrono::{DateTime, Utc};
use http::HeaderName;

use g3_types::net::{EgressInfo, HttpHeaderMap, HttpHeaderValue, HttpServerId};

// chained final info header
const UPSTREAM_ID: &str = "x-bd-upstream-id";
const UPSTREAM_ADDR: &str = "x-bd-upstream-addr";
const OUTGOING_IP: &str = "x-bd-outgoing-ip";

// local info header (append)
const REMOTE_CONNECTION_INFO: &str = "x-bd-remote-connection-info";
const DYNAMIC_EGRESS_INFO: &str = "x-bd-dynamic-egress-info";

thread_local! {
    static TL_BUF: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(256));
}

fn set_value_for_remote_connection_info(
    v: &mut Vec<u8>,
    server_id: &HttpServerId,
    bind: Option<IpAddr>,
    local: Option<SocketAddr>,
    remote: Option<SocketAddr>,
    expire: &Option<DateTime<Utc>>,
) {
    v.extend_from_slice(server_id.as_bytes());
    if let Some(ip) = bind {
        let _ = write!(v, "; bind={ip}");
    }
    if let Some(addr) = local {
        let _ = write!(v, "; local={addr}");
    }
    if let Some(addr) = remote {
        let _ = write!(v, "; remote={addr}");
    }
    if let Some(expire) = expire {
        let _ = write!(
            v,
            "; expire={}",
            expire.format_with_items(g3_datetime::format::std::RFC3339_FIXED_MICROSECOND.iter())
        );
    }
}

pub(crate) fn remote_connection_info(
    server_id: &HttpServerId,
    bind: Option<IpAddr>,
    local: Option<SocketAddr>,
    remote: Option<SocketAddr>,
    expire: &Option<DateTime<Utc>>,
) -> String {
    let mut buf = Vec::<u8>::with_capacity(256);
    buf.extend_from_slice(b"X-BD-Remote-Connection-Info: ");
    set_value_for_remote_connection_info(&mut buf, server_id, bind, local, remote, expire);
    buf.extend_from_slice(b"\r\n");
    // we can make sure that the vec contains only UTF-8 chars
    unsafe { String::from_utf8_unchecked(buf) }
}

pub(crate) fn set_remote_connection_info(
    headers: &mut HttpHeaderMap,
    server_id: &HttpServerId,
    bind: Option<IpAddr>,
    local: Option<SocketAddr>,
    remote: Option<SocketAddr>,
    expire: &Option<DateTime<Utc>>,
) {
    TL_BUF.with(|buf| {
        let mut buf = buf.borrow_mut();
        set_value_for_remote_connection_info(buf.as_mut(), server_id, bind, local, remote, expire);
        headers.append(HeaderName::from_static(REMOTE_CONNECTION_INFO), unsafe {
            HttpHeaderValue::from_buf_unchecked(buf.clone())
        });
        buf.clear();
    })
}

fn set_value_for_dynamic_egress_info(
    v: &mut Vec<u8>,
    server_id: &HttpServerId,
    egress: &EgressInfo,
) {
    v.extend_from_slice(server_id.as_bytes());
    if let Some(isp) = &egress.isp {
        let _ = write!(v, "; isp={}", BASE64_STANDARD.encode(isp));
    }
    if let Some(ip) = &egress.ip {
        let _ = write!(v, "; ip={ip}");
    }
    if let Some(area) = &egress.area {
        let _ = write!(v, "; area={}", BASE64_STANDARD.encode(area.to_string()));
    }
}

pub(crate) fn dynamic_egress_info(server_id: &HttpServerId, egress: &EgressInfo) -> String {
    let mut buf = Vec::<u8>::with_capacity(256);
    buf.extend_from_slice(b"X-BD-Dynamic-Egress-Info: ");
    set_value_for_dynamic_egress_info(&mut buf, server_id, egress);
    buf.extend_from_slice(b"\r\n");
    // we can make sure that the vec contains only UTF-8 chars
    unsafe { String::from_utf8_unchecked(buf) }
}

pub(crate) fn set_dynamic_egress_info(
    headers: &mut HttpHeaderMap,
    server_id: &HttpServerId,
    egress: &EgressInfo,
) {
    TL_BUF.with(|buf| {
        let mut buf = buf.borrow_mut();
        set_value_for_dynamic_egress_info(buf.as_mut(), server_id, egress);
        headers.append(HeaderName::from_static(DYNAMIC_EGRESS_INFO), unsafe {
            HttpHeaderValue::from_buf_unchecked(buf.clone())
        });
        buf.clear()
    })
}

pub(crate) fn set_upstream_id(headers: &mut HttpHeaderMap, id: &HttpServerId) {
    if !headers.contains_key(HeaderName::from_static(UPSTREAM_ID)) {
        headers.append(HeaderName::from_static(UPSTREAM_ID), id.to_header_value());
    }
}

pub(crate) fn upstream_addr(addr: SocketAddr) -> String {
    // header name should sync with UPSTREAM_ADDR
    format!("X-BD-Upstream-Addr: {addr}\r\n")
}

pub(crate) fn set_upstream_addr(headers: &mut HttpHeaderMap, addr: SocketAddr) {
    if !headers.contains_key(HeaderName::from_static(UPSTREAM_ADDR)) {
        headers.append(HeaderName::from_static(UPSTREAM_ADDR), unsafe {
            HttpHeaderValue::from_string_unchecked(addr.to_string())
        });
    }
}

pub(crate) fn outgoing_ip(ip: IpAddr) -> String {
    // header name should sync with OUTGOING_IP
    format!("X-BD-Outgoing-IP: {ip}\r\n")
}

pub(crate) fn set_outgoing_ip(headers: &mut HttpHeaderMap, addr: SocketAddr) {
    if !headers.contains_key(HeaderName::from_static(OUTGOING_IP)) {
        headers.append(HeaderName::from_static(OUTGOING_IP), unsafe {
            HttpHeaderValue::from_string_unchecked(addr.ip().to_string())
        });
    }
}
