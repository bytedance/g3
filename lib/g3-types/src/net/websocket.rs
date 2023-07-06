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
