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

    pub fn set_inner(&mut self, inner: HeaderValue) {
        self.inner = inner;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_header_value_operations() {
        // basic constructors
        let static_value = HttpHeaderValue::from_static("static_value");
        assert_eq!(static_value.to_str(), "static_value");
        assert_eq!(static_value.as_bytes(), b"static_value");
        assert!(static_value.original_name().is_none());

        // unsafe constructors
        let unsafe_string_value =
            unsafe { HttpHeaderValue::from_string_unchecked("unsafe_string".to_string()) };
        assert_eq!(unsafe_string_value.to_str(), "unsafe_string");
        assert_eq!(unsafe_string_value.as_bytes(), b"unsafe_string");

        let unsafe_buf_value =
            unsafe { HttpHeaderValue::from_buf_unchecked(vec![b't', b'e', b's', b't']) };
        assert_eq!(unsafe_buf_value.to_str(), "test");
        assert_eq!(unsafe_buf_value.as_bytes(), b"test");

        // parsing from string
        let parsed_value: HttpHeaderValue = "parsed-value".parse().unwrap();
        assert_eq!(parsed_value.to_str(), "parsed-value");

        // invalid parsing
        assert!("invalid\nvalue".parse::<HttpHeaderValue>().is_err());

        // original name functionality
        let mut value_with_name = HttpHeaderValue::from_static("test");
        assert!(value_with_name.original_name().is_none());

        value_with_name.set_original_name("X-Custom-Header");
        assert_eq!(value_with_name.original_name(), Some("X-Custom-Header"));

        value_with_name.set_original_name("Another-Header");
        assert_eq!(value_with_name.original_name(), Some("Another-Header"));

        // value mutation
        let mut mutable_value = HttpHeaderValue::from_static("initial");
        assert_eq!(mutable_value.to_str(), "initial");

        mutable_value.set_static_value("updated");
        assert_eq!(mutable_value.to_str(), "updated");

        mutable_value.set_inner(HeaderValue::from_static("new_inner"));
        assert_eq!(mutable_value.to_str(), "new_inner");

        // accessors
        let accessor_value = HttpHeaderValue::from_static("access_test");
        assert_eq!(
            accessor_value.inner(),
            &HeaderValue::from_static("access_test")
        );
        assert_eq!(accessor_value.as_bytes(), b"access_test");
        assert_eq!(accessor_value.to_str(), "access_test");

        // non-ASCII characters
        let non_ascii_value =
            unsafe { HttpHeaderValue::from_string_unchecked("café".to_string()) };
        assert_eq!(non_ascii_value.to_str(), "café");

        // into_inner
        let into_inner_value = HttpHeaderValue::from_static("inner_test");
        let inner: HeaderValue = into_inner_value.into_inner();
        assert_eq!(inner, HeaderValue::from_static("inner_test"));

        // conversion traits
        let conversion_value = HttpHeaderValue::from_static("conversion_test");
        let header_value: HeaderValue = conversion_value.into();
        assert_eq!(header_value, HeaderValue::from_static("conversion_test"));

        let as_ref_value = HttpHeaderValue::from_static("as_ref_test");
        let ref_header_value: &HeaderValue = as_ref_value.as_ref();
        assert_eq!(ref_header_value, &HeaderValue::from_static("as_ref_test"));

        // write_to_buf with and without original name
        let mut buf = Vec::new();
        let header_name = HeaderName::from_static("content-type");

        // without original name
        let normal_value = HttpHeaderValue::from_static("application/json");
        normal_value.write_to_buf(&header_name, &mut buf);
        assert_eq!(buf, b"content-type: application/json\r\n");

        // with original name
        buf.clear();
        let mut value_with_orig = HttpHeaderValue::from_static("application/json");
        value_with_orig.set_original_name("Content-Type");
        value_with_orig.write_to_buf(&header_name, &mut buf);
        assert_eq!(buf, b"Content-Type: application/json\r\n");
    }
}
