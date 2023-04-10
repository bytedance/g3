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

use std::str::FromStr;

use bytes::{BufMut, Bytes};
use http::{HeaderName, HeaderValue};

#[derive(Clone)]
pub struct HttpHeaderValue {
    inner: Bytes,
    original_name: Option<String>,
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

    pub fn set_original_name(&mut self, name: String) {
        self.original_name = Some(name);
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
