/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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

// sticky session headers (DX)
const STICKY_SESSION: &str = "x-sticky-session";
const STICKY_EXPIRES_AT: &str = "x-sticky-expires-at";

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
    TL_BUF.with_borrow_mut(|buf| {
        set_value_for_remote_connection_info(buf, server_id, bind, local, remote, expire);
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
    if let Some(isp) = &egress.isp() {
        let _ = write!(v, "; isp={}", BASE64_STANDARD.encode(isp));
    }
    if let Some(ip) = &egress.ip() {
        let _ = write!(v, "; ip={ip}");
    }
    if let Some(area) = &egress.area() {
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
    TL_BUF.with_borrow_mut(|buf| {
        set_value_for_dynamic_egress_info(buf, server_id, egress);
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

pub(crate) fn sticky_session_line(enabled: bool) -> String {
    let v = if enabled { "on" } else { "off" };
    format!("X-Sticky-Session: {v}\r\n")
}

pub(crate) fn sticky_expires_at_line(dt: &DateTime<Utc>) -> String {
    let s = dt
        .format_with_items(g3_datetime::format::std::RFC3339_FIXED_MICROSECOND.iter())
        .to_string();
    format!("X-Sticky-Expires-At: {s}\r\n")
}

pub(crate) fn set_sticky_session(headers: &mut HttpHeaderMap, enabled: bool) {
    let v = if enabled { "on" } else { "off" };
    headers.append(HeaderName::from_static(STICKY_SESSION), unsafe {
        HttpHeaderValue::from_string_unchecked(v.to_string())
    });
}

pub(crate) fn set_sticky_expires_at(headers: &mut HttpHeaderMap, dt: &DateTime<Utc>) {
    let s = dt
        .format_with_items(g3_datetime::format::std::RFC3339_FIXED_MICROSECOND.iter())
        .to_string();
    headers.append(HeaderName::from_static(STICKY_EXPIRES_AT), unsafe {
        HttpHeaderValue::from_string_unchecked(s)
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderName;

    #[test]
    fn sticky_header_lines() {
        let on = sticky_session_line(true);
        assert!(on.to_ascii_lowercase().starts_with("x-sticky-session: on"));
        let ts = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
        let exp = sticky_expires_at_line(&ts);
        assert!(exp.to_ascii_lowercase().starts_with("x-sticky-expires-at:"));
    }

    #[test]
    fn set_sticky_headers() {
        let mut map = HttpHeaderMap::default();
        set_sticky_session(&mut map, true);
        assert!(map.contains_key(HeaderName::from_static(STICKY_SESSION)));
        let ts = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
        set_sticky_expires_at(&mut map, &ts);
        assert!(map.contains_key(HeaderName::from_static(STICKY_EXPIRES_AT)));
    }

    #[test]
    fn set_upstream_and_outgoing_headers_once() {
        use std::net::{IpAddr, Ipv4Addr, SocketAddr};
        let mut map = HttpHeaderMap::default();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10,0,0,1)), 8080);
        set_upstream_addr(&mut map, addr);
        set_upstream_addr(&mut map, addr); // should not duplicate
        let mut n = 0;
        for _ in map.get_all(HeaderName::from_static(UPSTREAM_ADDR)).iter() { n += 1; }
        assert_eq!(n, 1);

        let mut map2 = HttpHeaderMap::default();
        set_outgoing_ip(&mut map2, addr);
        set_outgoing_ip(&mut map2, addr);
        let mut n2 = 0;
        for _ in map2.get_all(HeaderName::from_static(OUTGOING_IP)).iter() { n2 += 1; }
        assert_eq!(n2, 1);
    }
}
