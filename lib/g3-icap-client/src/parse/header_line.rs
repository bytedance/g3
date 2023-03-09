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
