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

use ahash::AHashMap;
use http::{HeaderMap, HeaderName};

use super::HttpHeaderValue;

#[derive(Default, Clone)]
pub struct HttpHeaderMap {
    inner: AHashMap<HeaderName, Vec<HttpHeaderValue>>,
}

impl HttpHeaderMap {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn insert(
        &mut self,
        name: HeaderName,
        value: HttpHeaderValue,
    ) -> Option<Vec<HttpHeaderValue>> {
        self.inner.insert(name, vec![value])
    }

    pub fn append(&mut self, name: HeaderName, value: HttpHeaderValue) {
        self.inner.entry(name).or_insert(vec![]).push(value);
    }

    pub fn remove(&mut self, name: HeaderName) -> Option<Vec<HttpHeaderValue>> {
        self.inner.remove(name.as_str())
    }

    pub fn remove_entry(
        &mut self,
        name: &HeaderName,
    ) -> Option<(HeaderName, Vec<HttpHeaderValue>)> {
        self.inner.remove_entry(name.as_str())
    }

    pub fn contains_key(&self, name: HeaderName) -> bool {
        self.inner.contains_key(name.as_str())
    }

    pub fn get(&self, name: HeaderName) -> Option<&HttpHeaderValue> {
        self.inner.get(name.as_str()).and_then(|v| v.get(0))
    }

    pub fn get_all(&self, name: HeaderName) -> &[HttpHeaderValue] {
        self.inner
            .get(name.as_str())
            .map(|v| v.as_slice())
            .unwrap_or_default()
    }

    pub fn for_each<F>(&self, mut call: F)
    where
        F: FnMut(&HeaderName, &HttpHeaderValue),
    {
        self.inner.iter().for_each(|(name, values)| {
            for value in values {
                call(name, value)
            }
        });
    }

    pub fn to_h2_map(&self) -> HeaderMap {
        let mut h2_map = HeaderMap::new();
        self.for_each(|name, value| {
            h2_map.append(name, value.into());
        });
        h2_map
    }

    pub fn into_h2_map(mut self) -> HeaderMap {
        let mut h2_map = HeaderMap::new();
        for (name, values) in self.inner.drain() {
            for value in values {
                h2_map.append(name.clone(), value.into());
            }
        }
        h2_map
    }
}
