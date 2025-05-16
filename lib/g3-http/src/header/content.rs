/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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
