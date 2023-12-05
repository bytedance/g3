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

use g3_types::metrics::StaticMetricsTags;

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

    pub fn add_static_tags(&mut self, tags: &StaticMetricsTags) {
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
