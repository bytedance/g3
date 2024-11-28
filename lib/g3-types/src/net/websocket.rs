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
