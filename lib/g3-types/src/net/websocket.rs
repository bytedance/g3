/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use http::header::Drain;
use http::{HeaderMap, HeaderName, HeaderValue, Uri, header};

pub enum WebSocketSubProtocol {
    Mqtt,
    StompV10,
    StompV11,
    StompV12,
}

impl WebSocketSubProtocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            WebSocketSubProtocol::Mqtt => "mqtt",
            WebSocketSubProtocol::StompV10 => "v10.stomp",
            WebSocketSubProtocol::StompV11 => "v11.stomp",
            WebSocketSubProtocol::StompV12 => "v12.stomp",
        }
    }

    pub fn from_buf(buf: &[u8]) -> Option<Self> {
        match buf {
            b"mqtt" => Some(WebSocketSubProtocol::Mqtt),
            b"v10.stomp" => Some(WebSocketSubProtocol::StompV10),
            b"v11.stomp" => Some(WebSocketSubProtocol::StompV11),
            b"v12.stomp" => Some(WebSocketSubProtocol::StompV12),
            _ => None,
        }
    }
}

pub struct WebSocketNotes {
    uri: Uri,
    headers: HeaderMap,
}

impl WebSocketNotes {
    pub fn new(uri: Uri) -> Self {
        WebSocketNotes {
            uri,
            headers: HeaderMap::with_capacity(8),
        }
    }

    pub fn append_request_header(&mut self, name: &HeaderName, value: &HeaderValue) {
        match name {
            &header::HOST
            | &header::ORIGIN
            | &header::SEC_WEBSOCKET_KEY
            | &header::SEC_WEBSOCKET_VERSION => {
                self.headers.insert(name.clone(), value.clone());
            }
            _ => {}
        }
    }

    pub fn append_request_headers<T: Into<HeaderValue>>(&mut self, req_headers: Drain<T>) {
        let mut last_name: Option<HeaderName> = None;
        for (name, value) in req_headers {
            if name.is_some() {
                last_name = name;
            }
            let name = last_name.as_ref().unwrap();
            match name {
                &header::HOST
                | &header::ORIGIN
                | &header::SEC_WEBSOCKET_KEY
                | &header::SEC_WEBSOCKET_VERSION => {
                    self.headers.insert(name.clone(), value.into());
                }
                _ => {}
            }
        }
    }

    pub fn append_response_header(&mut self, name: &HeaderName, value: &HeaderValue) {
        match name {
            &header::SEC_WEBSOCKET_ACCEPT | &header::SEC_WEBSOCKET_PROTOCOL => {
                self.headers.insert(name.clone(), value.clone());
            }
            &header::SEC_WEBSOCKET_EXTENSIONS => {
                self.headers.append(name.clone(), value.clone());
            }
            _ => {}
        }
    }

    pub fn append_response_headers<T: Into<HeaderValue>>(&mut self, rsp_headers: Drain<T>) {
        let mut last_name: Option<HeaderName> = None;
        for (name, value) in rsp_headers {
            if name.is_some() {
                last_name = name;
            }
            let name = last_name.as_ref().unwrap();
            match name {
                &header::SEC_WEBSOCKET_ACCEPT | &header::SEC_WEBSOCKET_PROTOCOL => {
                    self.headers.insert(name.clone(), value.into());
                }
                &header::SEC_WEBSOCKET_EXTENSIONS => {
                    self.headers.append(name.clone(), value.into());
                }
                _ => {}
            }
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut vec = Vec::with_capacity(256);
        vec.extend_from_slice(self.uri.path().as_bytes());
        vec.extend_from_slice(b"\r\n");
        for (name, value) in &self.headers {
            vec.extend_from_slice(name.as_str().as_bytes());
            vec.extend_from_slice(b": ");
            vec.extend_from_slice(value.as_bytes());
            vec.extend_from_slice(b"\r\n");
        }
        vec
    }

    #[inline]
    pub fn resource_name(&self) -> &str {
        self.uri.path()
    }

    #[inline]
    pub fn origin(&self) -> Option<&HeaderValue> {
        self.headers.get(header::ORIGIN)
    }

    #[inline]
    pub fn sub_protocol(&self) -> Option<&HeaderValue> {
        self.headers.get(header::SEC_WEBSOCKET_PROTOCOL)
    }

    #[inline]
    pub fn version(&self) -> Option<&HeaderValue> {
        self.headers.get(header::SEC_WEBSOCKET_VERSION)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn websocket_sub_protocol_as_str() {
        assert_eq!(WebSocketSubProtocol::Mqtt.as_str(), "mqtt");
        assert_eq!(WebSocketSubProtocol::StompV10.as_str(), "v10.stomp");
        assert_eq!(WebSocketSubProtocol::StompV11.as_str(), "v11.stomp");
        assert_eq!(WebSocketSubProtocol::StompV12.as_str(), "v12.stomp");
    }

    #[test]
    fn websocket_notes_new() {
        let uri = Uri::from_static("/test");
        let notes = WebSocketNotes::new(uri.clone());
        assert_eq!(notes.uri, uri);
        assert_eq!(notes.headers.len(), 0);
        assert_eq!(notes.headers.capacity(), 12);
    }

    #[test]
    fn append_request_header_allowed() {
        let uri = Uri::from_static("/test");
        let mut notes = WebSocketNotes::new(uri);
        let name = header::HOST;
        let value = HeaderValue::from_static("example.com");
        notes.append_request_header(&name, &value);
        assert_eq!(notes.headers.get(header::HOST).unwrap(), "example.com");

        let name = header::ORIGIN;
        let value = HeaderValue::from_static("http://example.com");
        notes.append_request_header(&name, &value);
        assert_eq!(
            notes.headers.get(header::ORIGIN).unwrap(),
            "http://example.com"
        );

        let name = header::SEC_WEBSOCKET_KEY;
        let value = HeaderValue::from_static("key123");
        notes.append_request_header(&name, &value);
        assert_eq!(
            notes.headers.get(header::SEC_WEBSOCKET_KEY).unwrap(),
            "key123"
        );

        let name = header::SEC_WEBSOCKET_VERSION;
        let value = HeaderValue::from_static("13");
        notes.append_request_header(&name, &value);
        assert_eq!(
            notes.headers.get(header::SEC_WEBSOCKET_VERSION).unwrap(),
            "13"
        );
    }

    #[test]
    fn append_request_header_ignored() {
        let uri = Uri::from_static("/test");
        let mut notes = WebSocketNotes::new(uri);
        let name = HeaderName::from_static("content-type");
        let value = HeaderValue::from_static("text/plain");
        notes.append_request_header(&name, &value);
        assert_eq!(notes.headers.get("content-type"), None);
    }

    #[test]
    fn append_request_headers() {
        let uri = Uri::from_static("/test");
        let mut notes = WebSocketNotes::new(uri);
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("example.com"));
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"));
        headers.insert(
            header::SEC_WEBSOCKET_KEY,
            HeaderValue::from_static("key123"),
        );

        let drain = headers.drain();
        notes.append_request_headers(drain);
        assert_eq!(notes.headers.get(header::HOST).unwrap(), "example.com");
        assert_eq!(
            notes.headers.get(header::SEC_WEBSOCKET_KEY).unwrap(),
            "key123"
        );
        assert_eq!(notes.headers.get(header::CONTENT_TYPE), None);
    }

    #[test]
    fn append_request_headers_none_name() {
        let uri = Uri::from_static("/test");
        let mut notes = WebSocketNotes::new(uri);
        let mut headers = HeaderMap::new();
        headers.append(header::HOST, HeaderValue::from_static("example.com"));
        headers.append(header::HOST, HeaderValue::from_static("example.org"));
        let drain = headers.drain();
        notes.append_request_headers(drain);
        assert_eq!(notes.headers.get(header::HOST).unwrap(), "example.org");
    }

    #[test]
    fn append_response_header_insert() {
        let uri = Uri::from_static("/test");
        let mut notes = WebSocketNotes::new(uri);
        let name = header::SEC_WEBSOCKET_ACCEPT;
        let value = HeaderValue::from_static("accept123");
        notes.append_response_header(&name, &value);
        assert_eq!(
            notes.headers.get(header::SEC_WEBSOCKET_ACCEPT).unwrap(),
            "accept123"
        );

        let name = header::SEC_WEBSOCKET_PROTOCOL;
        let value = HeaderValue::from_static("mqtt");
        notes.append_response_header(&name, &value);
        assert_eq!(
            notes.headers.get(header::SEC_WEBSOCKET_PROTOCOL).unwrap(),
            "mqtt"
        );
    }

    #[test]
    fn append_response_header_append() {
        let uri = Uri::from_static("/test");
        let mut notes = WebSocketNotes::new(uri);
        let name = header::SEC_WEBSOCKET_EXTENSIONS;
        let value1 = HeaderValue::from_static("ext1");
        let value2 = HeaderValue::from_static("ext2");
        notes.append_response_header(&name, &value1);
        notes.append_response_header(&name, &value2);
        let values = notes
            .headers
            .get_all(header::SEC_WEBSOCKET_EXTENSIONS)
            .into_iter()
            .collect::<Vec<_>>();
        assert_eq!(values.len(), 2);
        assert!(values.contains(&&value1));
        assert!(values.contains(&&value2));
    }

    #[test]
    fn append_response_header_ignored() {
        let uri = Uri::from_static("/test");
        let mut notes = WebSocketNotes::new(uri);
        let name = HeaderName::from_static("content-type");
        let value = HeaderValue::from_static("text/plain");
        notes.append_response_header(&name, &value);
        assert_eq!(notes.headers.get("content-type"), None);
    }

    #[test]
    fn append_response_headers() {
        let uri = Uri::from_static("/test");
        let mut notes = WebSocketNotes::new(uri);
        let mut headers = HeaderMap::new();
        headers.insert(
            header::SEC_WEBSOCKET_ACCEPT,
            HeaderValue::from_static("accept123"),
        );
        headers.append(
            header::SEC_WEBSOCKET_EXTENSIONS,
            HeaderValue::from_static("ext1"),
        );
        headers.append(
            header::SEC_WEBSOCKET_EXTENSIONS,
            HeaderValue::from_static("ext2"),
        );
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"));

        let drain = headers.drain();
        notes.append_response_headers(drain);
        assert_eq!(
            notes.headers.get(header::SEC_WEBSOCKET_ACCEPT).unwrap(),
            "accept123"
        );
        let values = notes
            .headers
            .get_all(header::SEC_WEBSOCKET_EXTENSIONS)
            .into_iter()
            .collect::<Vec<_>>();
        assert_eq!(values.len(), 2);
        assert!(values.contains(&&HeaderValue::from_static("ext1")));
        assert!(values.contains(&&HeaderValue::from_static("ext2")));
        assert_eq!(notes.headers.get(header::CONTENT_TYPE), None);
    }

    #[test]
    fn append_response_headers_none_name() {
        let uri = Uri::from_static("/test");
        let mut notes = WebSocketNotes::new(uri);
        let mut headers = HeaderMap::new();
        headers.append(
            header::SEC_WEBSOCKET_EXTENSIONS,
            HeaderValue::from_static("ext1"),
        );
        headers.append(
            header::SEC_WEBSOCKET_EXTENSIONS,
            HeaderValue::from_static("ext2"),
        );
        let drain = headers.drain();
        notes.append_response_headers(drain);
        let values = notes
            .headers
            .get_all(header::SEC_WEBSOCKET_EXTENSIONS)
            .into_iter()
            .collect::<Vec<_>>();
        assert_eq!(values.len(), 2);
        assert!(values.contains(&&HeaderValue::from_static("ext1")));
        assert!(values.contains(&&HeaderValue::from_static("ext2")));
    }

    #[test]
    fn serialize() {
        let uri = Uri::from_static("/websocket");
        let mut notes = WebSocketNotes::new(uri);
        notes.append_request_header(&header::HOST, &HeaderValue::from_static("example.com"));
        notes.append_response_header(
            &header::SEC_WEBSOCKET_PROTOCOL,
            &HeaderValue::from_static("mqtt"),
        );
        let serialized = notes.serialize();
        let expected =
            b"/websocket\r\nhost: example.com\r\nsec-websocket-protocol: mqtt\r\n".to_vec();
        assert_eq!(serialized, expected);
    }

    #[test]
    fn serialize_empty() {
        let uri = Uri::from_static("/test");
        let notes = WebSocketNotes::new(uri);
        let serialized = notes.serialize();
        let expected = b"/test\r\n".to_vec();
        assert_eq!(serialized, expected);
    }

    #[test]
    fn resource_name() {
        let uri = Uri::from_static("/websocket/path");
        let notes = WebSocketNotes::new(uri);
        assert_eq!(notes.resource_name(), "/websocket/path");
    }

    #[test]
    fn origin() {
        let uri = Uri::from_static("/test");
        let mut notes = WebSocketNotes::new(uri);
        let origin = HeaderValue::from_static("http://example.com");
        notes.append_request_header(&header::ORIGIN, &origin);
        assert_eq!(notes.origin(), Some(&origin));
        let empty_notes = WebSocketNotes::new(Uri::from_static("/test"));
        assert_eq!(empty_notes.origin(), None);
    }

    #[test]
    fn sub_protocol() {
        let uri = Uri::from_static("/test");
        let mut notes = WebSocketNotes::new(uri);
        let protocol = HeaderValue::from_static("mqtt");
        notes.append_response_header(&header::SEC_WEBSOCKET_PROTOCOL, &protocol);
        assert_eq!(notes.sub_protocol(), Some(&protocol));
        let empty_notes = WebSocketNotes::new(Uri::from_static("/test"));
        assert_eq!(empty_notes.sub_protocol(), None);
    }

    #[test]
    fn version() {
        let uri = Uri::from_static("/test");
        let mut notes = WebSocketNotes::new(uri);
        let version = HeaderValue::from_static("13");
        notes.append_request_header(&header::SEC_WEBSOCKET_VERSION, &version);
        assert_eq!(notes.version(), Some(&version));
        let empty_notes = WebSocketNotes::new(Uri::from_static("/test"));
        assert_eq!(empty_notes.version(), None);
    }
}
