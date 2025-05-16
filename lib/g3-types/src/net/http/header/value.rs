/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use bytes::{BufMut, Bytes};
use http::{HeaderName, HeaderValue};

use super::HttpOriginalHeaderName;

#[derive(Clone)]
pub struct HttpHeaderValue {
    inner: Bytes,
    original_name: Option<HttpOriginalHeaderName>,
}

impl HttpHeaderValue {
    ///
    /// # Safety
    ///
    /// The caller should make sure the value string is valid
    pub unsafe fn from_string_unchecked(value: String) -> Self {
        HttpHeaderValue {
            inner: Bytes::from(value),
            original_name: None,
        }
    }

    ///
    /// # Safety
    ///
    /// The caller should make sure the buf is valid
    pub unsafe fn from_buf_unchecked(buf: Vec<u8>) -> Self {
        HttpHeaderValue {
            inner: Bytes::from(buf),
            original_name: None,
        }
    }

    pub fn from_static(value: &'static str) -> Self {
        HttpHeaderValue {
            inner: Bytes::from_static(value.as_bytes()),
            original_name: None,
        }
    }

    pub fn set_original_name(&mut self, name: &str) {
        self.original_name = Some(HttpOriginalHeaderName::from(name));
    }

    pub fn set_static_value(&mut self, value: &'static str) {
        self.inner = Bytes::from_static(value.as_bytes());
    }

    pub fn original_name(&self) -> Option<&str> {
        self.original_name.as_deref()
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.inner.as_ref()
    }

    pub fn to_str(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(self.inner.as_ref()) }
    }

    pub fn write_to_buf(&self, name: &HeaderName, buf: &mut Vec<u8>) {
        if let Some(name) = self.original_name() {
            buf.put_slice(name.as_bytes());
        } else {
            buf.put_slice(name.as_ref());
        }
        buf.put_slice(b": ");
        buf.put_slice(self.inner.as_ref());
        buf.put_slice(b"\r\n");
    }
}

impl FromStr for HttpHeaderValue {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        for b in s.as_bytes() {
            if !is_valid(*b) {
                return Err(());
            }
        }
        Ok(HttpHeaderValue {
            inner: Bytes::copy_from_slice(s.as_bytes()),
            original_name: None,
        })
    }
}

impl From<HttpHeaderValue> for HeaderValue {
    fn from(value: HttpHeaderValue) -> Self {
        unsafe { HeaderValue::from_maybe_shared_unchecked(value.inner) }
    }
}

impl From<&HttpHeaderValue> for HeaderValue {
    fn from(value: &HttpHeaderValue) -> Self {
        unsafe { HeaderValue::from_maybe_shared_unchecked(value.inner.clone()) }
    }
}

#[inline]
fn is_valid(b: u8) -> bool {
    b >= 32 && b != 127 || b == b'\t'
}
