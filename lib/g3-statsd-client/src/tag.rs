/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_types::metrics::MetricTagMap;

#[derive(Clone, Default)]
pub struct StatsdTagGroup {
    buf: Vec<u8>,
}

impl StatsdTagGroup {
    pub fn add_tag<T: AsRef<str>>(&mut self, key: &str, value: T) {
        if !self.buf.is_empty() {
            self.buf.push(b',');
        }
        self.buf.extend_from_slice(key.as_bytes());
        self.buf.push(b':');
        self.buf.extend_from_slice(value.as_ref().as_bytes());
    }

    pub fn add_static_tags(&mut self, tags: &MetricTagMap) {
        for (k, v) in tags.iter() {
            self.add_tag(k.as_str(), v);
        }
    }

    pub fn add_tag_value<T: AsRef<str>>(&mut self, value: T) {
        if !self.buf.is_empty() {
            self.buf.push(b',');
        }
        self.buf.extend_from_slice(value.as_ref().as_bytes());
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.buf.len()
    }

    #[inline]
    pub(crate) fn as_bytes(&self) -> &[u8] {
        self.buf.as_slice()
    }
}
