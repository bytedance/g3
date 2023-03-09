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

use chrono::format::{parse, Parsed};
use chrono::{DateTime, ParseResult, Utc};

#[inline]
pub(crate) fn parse_from_str(s: &str) -> ParseResult<DateTime<Utc>> {
    let mut parsed = Parsed::new();
    parse(&mut parsed, s, g3_datetime::format::ftp::RFC3659.iter())?;
    parsed.to_datetime_with_timezone(&Utc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};

    #[test]
    fn parse_no_dot() {
        let dt = parse_from_str("20211201102030").unwrap();
        let expected = DateTime::parse_from_rfc3339("2021-12-01T10:20:30+00:00").unwrap();
        assert_eq!(dt, expected.with_timezone(&Utc));
    }

    #[test]
    fn parse_dot_1() {
        let dt = parse_from_str("20211201102030.1").unwrap();
        let expected = DateTime::parse_from_rfc3339("2021-12-01T10:20:30.1+00:00").unwrap();
        assert_eq!(dt, expected.with_timezone(&Utc));
    }

    #[test]
    fn parse_dot_3() {
        let dt = parse_from_str("20211201102030.123").unwrap();
        let expected = DateTime::parse_from_rfc3339("2021-12-01T10:20:30.123+00:00").unwrap();
        assert_eq!(dt, expected.with_timezone(&Utc));
    }
}
