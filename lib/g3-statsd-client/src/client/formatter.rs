/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use itoa::Integer;
use ryu::Float;
use smallvec::SmallVec;

use super::StatsdClient;
use crate::StatsdTagGroup;

enum MetricType {
    Count,
    Gauge,
}

impl MetricType {
    fn as_str(&self) -> &'static str {
        match self {
            MetricType::Count => "c",
            MetricType::Gauge => "g",
        }
    }
}

pub struct MetricFormatter<'a> {
    client: &'a mut StatsdClient,
    metric_type: MetricType,
    name: &'a str,
    value: SmallVec<[u8; 16]>,
    common_tags: Option<&'a StatsdTagGroup>,
    local_tags: StatsdTagGroup,

    has_tags: bool,
}

impl StatsdClient {
    pub fn count<'a, T: Integer>(&'a mut self, name: &'a str, value: T) -> MetricFormatter<'a> {
        let mut buffer = itoa::Buffer::new();
        let value = buffer.format(value);
        self.metric_with_type(
            MetricType::Count,
            name,
            SmallVec::from_slice(value.as_bytes()),
        )
    }

    pub fn count_with_tags<'a, T: Integer>(
        &'a mut self,
        name: &'a str,
        value: T,
        common_tags: &'a StatsdTagGroup,
    ) -> MetricFormatter<'a> {
        self.count(name, value).with_tag_group(common_tags)
    }

    pub fn gauge<'a, T: Integer>(&'a mut self, name: &'a str, value: T) -> MetricFormatter<'a> {
        let mut buffer = itoa::Buffer::new();
        let value = buffer.format(value);
        self.metric_with_type(
            MetricType::Gauge,
            name,
            SmallVec::from_slice(value.as_bytes()),
        )
    }

    pub fn gauge_with_tags<'a, T: Integer>(
        &'a mut self,
        name: &'a str,
        value: T,
        common_tags: &'a StatsdTagGroup,
    ) -> MetricFormatter<'a> {
        self.gauge(name, value).with_tag_group(common_tags)
    }

    pub fn gauge_float<'a, T: Float>(&'a mut self, name: &'a str, value: T) -> MetricFormatter<'a> {
        let mut buffer = ryu::Buffer::new();
        let value = buffer.format(value);
        self.metric_with_type(
            MetricType::Gauge,
            name,
            SmallVec::from_slice(value.as_bytes()),
        )
    }

    pub fn gauge_float_with_tags<'a, T: Float>(
        &'a mut self,
        name: &'a str,
        value: T,
        common_tags: &'a StatsdTagGroup,
    ) -> MetricFormatter<'a> {
        self.gauge_float(name, value).with_tag_group(common_tags)
    }

    fn metric_with_type<'a>(
        &'a mut self,
        metric_type: MetricType,
        name: &'a str,
        value: SmallVec<[u8; 16]>,
    ) -> MetricFormatter<'a> {
        let has_tags = self.tags.len() > 0;
        MetricFormatter {
            client: self,
            metric_type,
            name,
            value,
            common_tags: None,
            local_tags: StatsdTagGroup::default(),
            has_tags,
        }
    }
}

impl<'a> MetricFormatter<'a> {
    fn with_tag_group(mut self, tags: &'a StatsdTagGroup) -> Self {
        let tags_len = tags.len();
        if tags_len > 0 {
            self.has_tags = true;
            self.common_tags = Some(tags);
        }
        self
    }

    pub fn with_tag<T: AsRef<str>>(mut self, key: &str, value: T) -> Self {
        // set has_tags later when send
        self.local_tags.add_tag(key, value);
        self
    }

    pub fn with_tag_value<T: AsRef<str>>(mut self, value: T) -> Self {
        // set has_tags later when send
        self.local_tags.add_tag_value(value);
        self
    }

    pub fn send(mut self) {
        if self.local_tags.len() > 0 {
            self.has_tags = true;
        }
        if let Err(e) = self.client.sink.emit(|buf| {
            if !self.client.prefix.is_empty() {
                buf.extend_from_slice(self.client.prefix.as_bytes());
                buf.push(b'.');
            }
            buf.extend_from_slice(self.name.as_bytes());
            buf.push(b':');
            buf.extend_from_slice(self.value.as_slice());
            buf.push(b'|');
            buf.extend_from_slice(self.metric_type.as_str().as_bytes());

            if self.has_tags {
                buf.extend_from_slice(b"|#");
            } else {
                buf.push(b'\n');
                return;
            }

            let mut append_tags = false;
            if self.client.tags.len() > 0 {
                buf.extend_from_slice(self.client.tags.as_bytes());
                append_tags = true;
            }

            if let Some(common_tags) = self.common_tags
                && common_tags.len() > 0
            {
                if append_tags {
                    buf.push(b',');
                }
                buf.extend_from_slice(common_tags.as_bytes());
                append_tags = true;
            }

            if self.local_tags.len() > 0 {
                if append_tags {
                    buf.push(b',');
                }
                buf.extend_from_slice(self.local_tags.as_bytes());
            }

            buf.push(b'\n');
        }) {
            self.client.handle_emit_error(e);
        }
    }
}
