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

use mime::Mime;

pub fn content_length(len: u64) -> String {
    format!("Content-Length: {len}\r\n")
}

pub fn content_type(mime: &Mime) -> String {
    format!("Content-Type: {mime}\r\n")
}

pub fn content_range_sized(start: u64, end: u64, total: u64) -> String {
    format!("Content-Range: bytes {start}-{end}/{total}\r\n")
}

pub fn content_range_overflowed(start: u64) -> String {
    format!("Content-Range: bytes */{start}\r\n")
}
