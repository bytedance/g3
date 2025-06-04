/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use bytes::{BufMut, Bytes};
use http::header::InvalidHeaderValue;
use http::{HeaderName, HeaderValue};

use super::HttpOriginalHeaderName;

#[derive(Debug, Clone)]
pub struct HttpHeaderValue {
    inner: HeaderValue,
    original_name: Option<HttpOriginalHeaderName>,
}

impl HttpHeaderValue {
    ///
    /// # Safety
    ///
    /// The caller should make sure the value string is valid
    pub unsafe fn from_string_unchecked(value: String) -> Self {
        HttpHeaderValue {
            inner: unsafe { HeaderValue::from_maybe_shared_unchecked(Bytes::from(value)) },
            original_name: None,
        }
    }

    ///
    /// # Safety
    ///
    /// The caller should make sure the buf is valid
    pub unsafe fn from_buf_unchecked(buf: Vec<u8>) -> Self {
        HttpHeaderValue {
            inner: unsafe { HeaderValue::from_maybe_shared_unchecked(Bytes::from(buf)) },
            original_name: None,
        }
    }

    pub fn from_static(value: &'static str) -> Self {
        HttpHeaderValue {
            inner: HeaderValue::from_static(value),
            original_name: None,
        }
    }

    pub fn set_original_name(&mut self, name: &str) {
        self.original_name = Some(HttpOriginalHeaderName::from(name));
    }

    pub fn set_static_value(&mut self, value: &'static str) {
        self.inner = HeaderValue::from_static(value);
    }

    #[inline]
    pub fn inner(&self) -> &HeaderValue {
        &self.inner
    }

    #[inline]
    pub fn into_inner(self) -> HeaderValue {
        self.inner
    }

    pub fn original_name(&self) -> Option<&str> {
        self.original_name.as_deref()
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.inner.as_bytes()
    }

    pub fn to_str(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(self.inner.as_bytes()) }
    }

    pub fn write_to_buf(&self, name: &HeaderName, buf: &mut Vec<u8>) {
        if let Some(name) = self.original_name() {
            buf.put_slice(name.as_bytes());
        } else {
            buf.put_slice(name.as_ref());
        }
        buf.put_slice(b": ");
        buf.put_slice(self.inner.as_bytes());
        buf.put_slice(b"\r\n");
    }
}

impl FromStr for HttpHeaderValue {
    type Err = InvalidHeaderValue;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(HttpHeaderValue {
            inner: HeaderValue::from_str(s)?,
            original_name: None,
        })
    }
}

impl From<HttpHeaderValue> for HeaderValue {
    fn from(value: HttpHeaderValue) -> Self {
        value.into_inner()
    }
}

impl AsRef<HeaderValue> for HttpHeaderValue {
    fn as_ref(&self) -> &HeaderValue {
        self.inner()
    }
}
