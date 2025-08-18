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

#[cfg(test)]
mod tests {
    use super::*;
    use mime::{APPLICATION_JSON, TEXT_PLAIN};

    #[test]
    fn t_content_length() {
        // Normal values
        assert_eq!(content_length(100), "Content-Length: 100\r\n");
        // Zero length
        assert_eq!(content_length(0), "Content-Length: 0\r\n");
        // Maximum u64 value
        assert_eq!(
            content_length(u64::MAX),
            format!("Content-Length: {}\r\n", u64::MAX)
        );
    }

    #[test]
    fn t_content_type() {
        // Plain text type
        assert_eq!(content_type(&TEXT_PLAIN), "Content-Type: text/plain\r\n");
        // JSON type
        assert_eq!(
            content_type(&APPLICATION_JSON),
            "Content-Type: application/json\r\n"
        );
    }

    #[test]
    fn t_content_range_sized() {
        // Standard range
        assert_eq!(
            content_range_sized(0, 99, 100),
            "Content-Range: bytes 0-99/100\r\n"
        );
        // Single byte range
        assert_eq!(
            content_range_sized(0, 0, 1),
            "Content-Range: bytes 0-0/1\r\n"
        );
        // Large values
        assert_eq!(
            content_range_sized(u64::MAX - 1, u64::MAX, u64::MAX),
            format!(
                "Content-Range: bytes {}-{}/{}\r\n",
                u64::MAX - 1,
                u64::MAX,
                u64::MAX
            )
        );
    }

    #[test]
    fn t_content_range_overflowed() {
        // Zero start
        assert_eq!(content_range_overflowed(0), "Content-Range: bytes */0\r\n");
        // Normal value
        assert_eq!(
            content_range_overflowed(100),
            "Content-Range: bytes */100\r\n"
        );
        // Maximum u64 value
        assert_eq!(
            content_range_overflowed(u64::MAX),
            format!("Content-Range: bytes */{}\r\n", u64::MAX)
        );
    }
}
