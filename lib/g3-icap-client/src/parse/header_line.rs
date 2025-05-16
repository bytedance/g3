/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::IcapLineParseError;

pub(crate) struct HeaderLine<'a> {
    pub(crate) name: &'a str,
    pub(crate) value: &'a str,
}

impl<'a> HeaderLine<'a> {
    pub(crate) fn parse(buf: &'a [u8]) -> Result<HeaderLine<'a>, IcapLineParseError> {
        let line = std::str::from_utf8(buf)?;

        let p = memchr::memchr(b':', line.as_bytes())
            .ok_or(IcapLineParseError::NoDelimiterFound(':'))?;
        if p == 0 {
            return Err(IcapLineParseError::MissingHeaderName);
        }

        let name = &line[0..p];
        let value = line[p + 1..].trim();
        Ok(HeaderLine { name, value })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding() {
        let s = "测试: 结果\r\n";
        let header = HeaderLine::parse(s.as_bytes()).unwrap();

        assert_eq!(header.name, "测试");
        assert_eq!(header.value, "结果");
    }
}
