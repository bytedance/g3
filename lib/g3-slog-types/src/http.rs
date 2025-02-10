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

use std::fmt;

use h2::StreamId;
use http::{HeaderValue, Method, Uri};
use slog::{Key, Record, Serializer, Value};

pub struct LtHttpMethod<'a>(pub &'a Method);

impl Value for LtHttpMethod<'_> {
    fn serialize(
        &self,
        _record: &Record,
        key: slog::Key,
        serializer: &mut dyn Serializer,
    ) -> slog::Result {
        serializer.emit_str(key, self.0.as_str())
    }
}

pub struct LtHttpUri<'a> {
    uri: &'a Uri,
    max_chars: usize,
}

impl<'a> LtHttpUri<'a> {
    pub fn new(uri: &'a Uri, max_chars: usize) -> Self {
        LtHttpUri { uri, max_chars }
    }

    fn len(&self) -> usize {
        let mut len = self.uri.path().len();

        if let Some(scheme) = self.uri.scheme() {
            len += scheme.as_str().len() + 3; // include "://"
        }

        if let Some(authority) = self.uri.authority() {
            len += authority.as_str().len() + 3; // include "xyz"
        }

        if let Some(query) = self.uri.query() {
            len += query.len() + 1; // include "?"
        }

        len
    }
}

impl fmt::Display for LtHttpUri<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(scheme) = self.uri.scheme() {
            write!(f, "{scheme}://")?;
        }

        if let Some(authority) = self.uri.authority() {
            let s = authority.as_str();

            if let Some(at_pos) = memchr::memchr(b'@', s.as_bytes()) {
                if let Some(p) = memchr::memchr(b':', &s.as_bytes()[0..at_pos]) {
                    write!(f, "{}", &s[0..=p])?;
                    write!(f, "xyz{}", &s[at_pos..])?;
                } else {
                    write!(f, "{authority}")?;
                }
            } else {
                write!(f, "{authority}")?;
            }
        }

        write!(f, "{}", self.uri.path())?;

        if let Some(query) = self.uri.query() {
            write!(f, "?{query}")?;
        }

        Ok(())
    }
}

impl Value for LtHttpUri<'_> {
    fn serialize(
        &self,
        _record: &Record,
        key: slog::Key,
        serializer: &mut dyn Serializer,
    ) -> slog::Result {
        if self.len() < self.max_chars {
            serializer.emit_arguments(key, &format_args!("{self}"))
        } else {
            let uri = self.to_string();
            if let Some((i, _)) = uri.char_indices().nth(self.max_chars) {
                serializer.emit_str(key, uri.get(..i).unwrap_or(&uri))
            } else {
                serializer.emit_str(key, &uri)
            }
        }
    }
}

pub struct LtHttpHeaderValue<'a>(pub &'a HeaderValue);

impl Value for LtHttpHeaderValue<'_> {
    fn serialize(
        &self,
        _record: &Record,
        key: Key,
        serializer: &mut dyn Serializer,
    ) -> slog::Result {
        match self.0.to_str() {
            Ok(v) => serializer.emit_str(key, v),
            Err(e) => serializer.emit_arguments(key, &format_args!("invalid header value: {e}")),
        }
    }
}

pub struct LtH2StreamId<'a>(pub &'a StreamId);

impl Value for LtH2StreamId<'_> {
    fn serialize(
        &self,
        _record: &Record,
        key: slog::Key,
        serializer: &mut dyn Serializer,
    ) -> slog::Result {
        serializer.emit_u32(key, self.0.as_u32())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn format_uri() {
        let raw = "https://a:b@example.com/a?b=c";
        let uri = Uri::from_str(raw).unwrap();
        let lt_uri = LtHttpUri::new(&uri, 1024);
        let log1 = format!("{lt_uri}");
        assert_eq!("https://a:xyz@example.com/a?b=c", log1);
    }
}
