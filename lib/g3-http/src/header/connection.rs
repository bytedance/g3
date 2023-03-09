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

use http::HeaderName;

pub const fn connection_as_bytes(close: bool) -> &'static [u8] {
    if close {
        b"Connection: Close\r\n"
    } else {
        b"Connection: Keep-Alive\r\n"
    }
}

pub fn connection_with_more_headers(close: bool, headers: &[HeaderName]) -> String {
    let mut s = String::with_capacity(32);
    if close {
        s.push_str("Connection: Close");
    } else {
        s.push_str("Connection: Keep-Alive");
    }
    for h in headers {
        s.push_str(", ");
        s.push_str(h.as_str());
    }
    s.push_str("\r\n");
    s
}
