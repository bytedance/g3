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

use std::collections::BTreeMap;
use std::fmt::{self, Write};
use std::str::FromStr;

use anyhow::anyhow;

use g3_types::metrics::{MetricTagName, MetricTagValue};

#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MetricTagMap {
    inner: BTreeMap<MetricTagName, MetricTagValue>,
}

impl MetricTagMap {
    #[cfg(test)]
    pub(crate) fn get(&self, key: &MetricTagName) -> Option<&MetricTagValue> {
        self.inner.get(key)
    }

    pub(crate) fn drop(&mut self, name: &MetricTagName) {
        self.inner.remove(name);
    }

    pub(crate) fn parse_statsd(&mut self, data: &[u8]) -> anyhow::Result<()> {
        let iter = TagKvIter::new(data, b',', b':');
        for r in iter {
            let (name, value) = r?;
            self.inner.insert(name, value);
        }
        Ok(())
    }

    pub(crate) fn display_graphite(&self) -> DisplayTagMap<'_> {
        DisplayTagMap {
            inner: self,
            assign_delimiter: '=',
            next_delimiter: ';',
        }
    }

    #[allow(unused)]
    pub(crate) fn display_influxdb(&self) -> DisplayTagMap<'_> {
        DisplayTagMap {
            inner: self,
            assign_delimiter: '=',
            next_delimiter: ',',
        }
    }

    pub(crate) fn display_opentsdb(&self) -> DisplayTagMap<'_> {
        DisplayTagMap {
            inner: self,
            assign_delimiter: '=',
            next_delimiter: ' ',
        }
    }

    #[allow(unused)]
    pub(crate) fn display_statsd(&self) -> DisplayTagMap<'_> {
        DisplayTagMap {
            inner: self,
            assign_delimiter: ':',
            next_delimiter: ',',
        }
    }
}

pub(crate) struct DisplayTagMap<'a> {
    inner: &'a MetricTagMap,
    assign_delimiter: char,
    next_delimiter: char,
}

impl fmt::Display for DisplayTagMap<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut iter = self.inner.inner.iter();
        let Some((name, value)) = iter.next() else {
            return Ok(());
        };
        f.write_str(name.as_str())?;
        f.write_char(self.assign_delimiter)?;
        f.write_str(value.as_str())?;

        for (name, value) in iter {
            f.write_char(self.next_delimiter)?;
            f.write_str(name.as_str())?;
            f.write_char(self.assign_delimiter)?;
            f.write_str(value.as_str())?;
        }
        Ok(())
    }
}

struct TagKvIter<'a> {
    data: &'a [u8],
    assign_delimiter: u8,
    next_delimiter: u8,
    offset: usize,
}

impl<'a> TagKvIter<'a> {
    fn new(data: &'a [u8], next_delimiter: u8, assign_delimiter: u8) -> Self {
        TagKvIter {
            data,
            assign_delimiter,
            next_delimiter,
            offset: 0,
        }
    }

    fn next_field(&mut self) -> Option<&'a [u8]> {
        if self.offset >= self.data.len() {
            return None;
        }

        let left = &self.data[self.offset..];
        match memchr::memchr(self.next_delimiter, left) {
            Some(p) => {
                self.offset += p + 1;
                Some(&left[..p])
            }
            None => {
                self.offset = self.data.len();
                Some(left)
            }
        }
    }
}

impl Iterator for TagKvIter<'_> {
    type Item = anyhow::Result<(MetricTagName, MetricTagValue)>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let part = self.next_field()?;
            if part.is_empty() {
                continue;
            }

            return match memchr::memchr(self.assign_delimiter, part) {
                Some(p) => match parse_tag_name(&part[..p]) {
                    Ok(name) => {
                        if p + 1 >= part.len() {
                            Some(Ok((name, MetricTagValue::EMPTY)))
                        } else {
                            match parse_tag_value(&part[p + 1..]) {
                                Ok(value) => Some(Ok((name, value))),
                                Err(e) => Some(Err(e)),
                            }
                        }
                    }
                    Err(e) => Some(Err(e)),
                },
                None => match parse_tag_name(part) {
                    Ok(name) => Some(Ok((name, MetricTagValue::EMPTY))),
                    Err(e) => Some(Err(e)),
                },
            };
        }
    }
}

fn parse_tag_name(buf: &[u8]) -> anyhow::Result<MetricTagName> {
    let name = std::str::from_utf8(buf).map_err(|e| anyhow!("invalid tag name: {e}"))?;
    MetricTagName::from_str(name).map_err(|e| anyhow!("invalid tag name: {e}"))
}

fn parse_tag_value(buf: &[u8]) -> anyhow::Result<MetricTagValue> {
    let value = std::str::from_utf8(buf).map_err(|e| anyhow!("invalid tag value: {e}"))?;
    MetricTagValue::from_str(value).map_err(|e| anyhow!("invalid tag value: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statsd() {
        let buf = b"daemon_group:test,server:test-tls,online:y,stat_id:406995395936281";
        let mut iter = TagKvIter::new(buf, b',', b':');
        let (name, value) = iter.next().unwrap().unwrap();
        assert_eq!(name.as_str(), "daemon_group");
        assert_eq!(value.as_str(), "test");

        let (name, value) = iter.next().unwrap().unwrap();
        assert_eq!(name.as_str(), "server");
        assert_eq!(value.as_str(), "test-tls");

        let (name, value) = iter.next().unwrap().unwrap();
        assert_eq!(name.as_str(), "online");
        assert_eq!(value.as_str(), "y");

        let (name, value) = iter.next().unwrap().unwrap();
        assert_eq!(name.as_str(), "stat_id");
        assert_eq!(value.as_str(), "406995395936281");
    }

    #[test]
    fn fmt() {
        let buf = b"daemon_group:test,server:test-tls,online:y,stat_id:406995395936281";
        let mut tag_map = MetricTagMap::default();
        tag_map.parse_statsd(buf).unwrap();

        assert_eq!(
            tag_map.display_graphite().to_string().as_str(),
            "daemon_group=test;online=y;server=test-tls;stat_id=406995395936281"
        );
        assert_eq!(
            tag_map.display_influxdb().to_string().as_str(),
            "daemon_group=test,online=y,server=test-tls,stat_id=406995395936281"
        );
        assert_eq!(
            tag_map.display_opentsdb().to_string().as_str(),
            "daemon_group=test online=y server=test-tls stat_id=406995395936281"
        );
        assert_eq!(
            tag_map.display_statsd().to_string().as_str(),
            "daemon_group:test,online:y,server:test-tls,stat_id:406995395936281"
        );
    }
}
