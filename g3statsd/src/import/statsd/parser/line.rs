/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use std::str::FromStr;
use std::sync::Arc;

use anyhow::anyhow;

use super::StatsdParseError;
use crate::types::{MetricName, MetricRecord, MetricTagMap, MetricType, MetricValue};

pub(super) struct LineParser<'a> {
    line: &'a [u8],
    offset: usize,
}

impl<'a> LineParser<'a> {
    pub(super) fn new(line: &'a [u8]) -> Self {
        LineParser { line, offset: 0 }
    }

    pub(super) fn parse(mut self) -> Result<LineValueIter<'a>, StatsdParseError> {
        let Some(part1) = self.next_part() else {
            return Err(StatsdParseError::NoName);
        };

        let Some(part2) = self.next_part() else {
            return Err(StatsdParseError::NoType);
        };
        let metric_type = parse_type(part2)?;

        let mut tag_map = MetricTagMap::default();
        while let Some(part) = self.next_part() {
            if part.is_empty() {
                continue;
            }

            match part[0] {
                b'@' => {} // sample rate
                b'#' => {
                    tag_map
                        .parse_buf(&part[1..], b':', b',')
                        .map_err(StatsdParseError::InvalidTagValue)?;
                }
                b'T' => {} // timestamp
                b'c' | b'e' => {}
                _ => {}
            }
        }

        LineValueIter::new(part1, metric_type, tag_map)
    }

    fn next_part(&mut self) -> Option<&'a [u8]> {
        if self.offset >= self.line.len() {
            return None;
        }
        let left = &self.line[self.offset..];
        match memchr::memchr(b'|', left) {
            Some(p) => {
                self.offset += p + 1;
                Some(&left[..p])
            }
            None => {
                self.offset = self.line.len();
                Some(left)
            }
        }
    }
}

pub(super) struct LineValueIter<'a> {
    r#type: MetricType,
    name: Arc<MetricName>,
    tag_map: Arc<MetricTagMap>,
    value_buf: &'a [u8],
    offset: usize,
}

impl<'a> LineValueIter<'a> {
    fn new(
        part: &'a [u8],
        r#type: MetricType,
        tag_map: MetricTagMap,
    ) -> Result<LineValueIter<'a>, StatsdParseError> {
        let Some(p) = memchr::memchr(b':', part) else {
            return Err(StatsdParseError::NoValue);
        };
        if p == 0 {
            return Err(StatsdParseError::NoName);
        }
        if p + 1 >= part.len() {
            return Err(StatsdParseError::NoValue);
        }

        let name = &part[..p];
        let name = std::str::from_utf8(name)
            .map_err(|e| StatsdParseError::InvalidName(anyhow::Error::new(e)))?;
        let name = MetricName::parse(name).map_err(|e| {
            StatsdParseError::InvalidName(anyhow!("invalid node name in name field: {e}"))
        })?;

        Ok(LineValueIter {
            r#type,
            name: Arc::new(name),
            tag_map: Arc::new(tag_map),
            value_buf: &part[p + 1..],
            offset: 0,
        })
    }

    fn next_value(&mut self) -> Option<&'a [u8]> {
        if self.offset >= self.value_buf.len() {
            return None;
        }
        let left = &self.value_buf[self.offset..];
        match memchr::memchr(b':', left) {
            Some(p) => {
                self.offset += p + 1;
                Some(&left[..p])
            }
            None => {
                self.offset = self.value_buf.len();
                Some(left)
            }
        }
    }
}

impl Iterator for LineValueIter<'_> {
    type Item = Result<MetricRecord, StatsdParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let value = self.next_value()?;
            if value.is_empty() {
                continue;
            }

            return match std::str::from_utf8(value) {
                Ok(s) => match MetricValue::from_str(s) {
                    Ok(value) => Some(Ok(MetricRecord {
                        r#type: self.r#type,
                        name: self.name.clone(),
                        tag_map: self.tag_map.clone(),
                        value,
                    })),
                    Err(e) => Some(Err(StatsdParseError::InvalidValue(e))),
                },
                Err(e) => Some(Err(StatsdParseError::InvalidValue(anyhow::Error::new(e)))),
            };
        }
    }
}

fn parse_type(part: &[u8]) -> Result<MetricType, StatsdParseError> {
    match part.len() {
        0 => Err(StatsdParseError::NoType),
        1 => match part[0] {
            b'c' => Ok(MetricType::Counter),
            b'g' => Ok(MetricType::Gauge),
            _ => Err(StatsdParseError::UnsupportedType),
        },
        _ => Err(StatsdParseError::UnsupportedType),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_types::metrics::MetricTagName;

    #[test]
    fn etsy_statsd() {
        let counter = b"gorets:1|c";
        let parser = LineParser::new(counter);
        let mut iter = parser.parse().unwrap();
        let r = iter.next().unwrap().unwrap();
        assert_eq!(r.r#type, MetricType::Counter);
        assert_eq!(r.value, MetricValue::Unsigned(1));
        assert!(r.name.display('.').to_string().as_bytes().eq(b"gorets"));

        let counter = b"gorets:-1|c|@0.1";
        let parser = LineParser::new(counter);
        let mut iter = parser.parse().unwrap();
        let r = iter.next().unwrap().unwrap();
        assert_eq!(r.r#type, MetricType::Counter);
        assert_eq!(r.value, MetricValue::Signed(-1));
        assert!(r.name.display('.').to_string().as_bytes().eq(b"gorets"));

        let gauge = b"gaugor:333|g";
        let parser = LineParser::new(gauge);
        let mut iter = parser.parse().unwrap();
        let r = iter.next().unwrap().unwrap();
        assert_eq!(r.r#type, MetricType::Gauge);
        assert_eq!(r.value, MetricValue::Unsigned(333));
        assert!(r.name.display('.').to_string().as_bytes().eq(b"gaugor"));

        let gauge = b"gaugor:-10|g";
        let parser = LineParser::new(gauge);
        let mut iter = parser.parse().unwrap();
        let r = iter.next().unwrap().unwrap();
        assert_eq!(r.r#type, MetricType::Gauge);
        assert_eq!(r.value, MetricValue::Signed(-10));
        assert!(r.name.display('.').to_string().as_bytes().eq(b"gaugor"));
    }

    #[test]
    fn dog_statsd() {
        let counter = b"users.online:1|c|@0.5|#country:china";
        let parser = LineParser::new(counter);
        let mut iter = parser.parse().unwrap();
        let r = iter.next().unwrap().unwrap();
        assert_eq!(r.r#type, MetricType::Counter);
        assert_eq!(r.value, MetricValue::Unsigned(1));
        assert!(
            r.name
                .display('.')
                .to_string()
                .as_bytes()
                .eq(b"users.online")
        );
        let tag_name = unsafe { MetricTagName::new_static_unchecked("country") };
        let tag_v = r.tag_map.get(&tag_name).unwrap();
        assert_eq!(tag_v.as_str(), "china");

        let counter = b"fuel.level:0.5|g";
        let parser = LineParser::new(counter);
        let mut iter = parser.parse().unwrap();
        let r = iter.next().unwrap().unwrap();
        assert_eq!(r.r#type, MetricType::Gauge);
        assert_eq!(r.value, MetricValue::Double(0.5));
        assert!(r.name.display('.').to_string().as_bytes().eq(b"fuel.level"));
    }

    #[test]
    fn dog_statsd_1_1() {
        let counter = b"users.online:1:2:3|c|@0.5|#country:china";
        let parser = LineParser::new(counter);
        let mut iter = parser.parse().unwrap();
        let r1 = iter.next().unwrap().unwrap();
        assert_eq!(r1.r#type, MetricType::Counter);
        assert_eq!(r1.value, MetricValue::Unsigned(1));
        assert!(
            r1.name
                .display('.')
                .to_string()
                .as_bytes()
                .eq(b"users.online")
        );
        let tag_name = unsafe { MetricTagName::new_static_unchecked("country") };
        let tag_v = r1.tag_map.get(&tag_name).unwrap();
        assert_eq!(tag_v.as_str(), "china");

        let r2 = iter.next().unwrap().unwrap();
        assert_eq!(r2.r#type, MetricType::Counter);
        assert_eq!(r2.value, MetricValue::Unsigned(2));
        assert!(
            r2.name
                .display('.')
                .to_string()
                .as_bytes()
                .eq(b"users.online")
        );
        let tag_v = r2.tag_map.get(&tag_name).unwrap();
        assert_eq!(tag_v.as_str(), "china");

        let r3 = iter.next().unwrap().unwrap();
        assert_eq!(r3.r#type, MetricType::Counter);
        assert_eq!(r3.value, MetricValue::Unsigned(3));
    }
}
