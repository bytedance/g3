/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use http::header::{AsHeaderName, Drain, GetAll};
use http::{HeaderMap, HeaderName};

use super::HttpHeaderValue;

#[derive(Debug, Default, Clone)]
pub struct HttpHeaderMap {
    inner: HeaderMap<HttpHeaderValue>,
}

impl HttpHeaderMap {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    #[inline]
    pub fn insert(&mut self, name: HeaderName, value: HttpHeaderValue) -> Option<HttpHeaderValue> {
        self.inner.insert(name, value)
    }

    #[inline]
    pub fn append(&mut self, name: HeaderName, value: HttpHeaderValue) {
        self.inner.append(name, value);
    }

    #[inline]
    pub fn remove<K: AsHeaderName>(&mut self, name: K) -> Option<HttpHeaderValue> {
        self.inner.remove(name)
    }

    #[inline]
    pub fn contains_key<K: AsHeaderName>(&self, name: K) -> bool {
        self.inner.contains_key(name)
    }

    #[inline]
    pub fn get<K: AsHeaderName>(&self, name: K) -> Option<&HttpHeaderValue> {
        self.inner.get(name)
    }

    #[inline]
    pub fn get_mut<K: AsHeaderName>(&mut self, name: K) -> Option<&mut HttpHeaderValue> {
        self.inner.get_mut(name)
    }

    #[inline]
    pub fn get_all<K: AsHeaderName>(&self, name: K) -> GetAll<'_, HttpHeaderValue> {
        self.inner.get_all(name)
    }

    pub fn for_each<F>(&self, mut call: F)
    where
        F: FnMut(&HeaderName, &HttpHeaderValue),
    {
        self.inner
            .iter()
            .for_each(|(name, value)| call(name, value));
    }

    pub fn drain(&mut self) -> Drain<'_, HttpHeaderValue> {
        self.inner.drain()
    }
}

impl From<HttpHeaderMap> for HeaderMap {
    fn from(mut value: HttpHeaderMap) -> Self {
        let mut new_map = HeaderMap::with_capacity(value.inner.capacity());

        let mut last_name: Option<HeaderName> = None;
        for (name, value) in value.inner.drain() {
            match name {
                Some(name) => {
                    last_name = Some(name.clone());
                    new_map.append(name, value.into_inner());
                }
                None => {
                    let Some(name) = &last_name else {
                        break;
                    };
                    new_map.append(name, value.into_inner());
                }
            }
        }
        new_map
    }
}

impl From<&HttpHeaderMap> for HeaderMap {
    fn from(value: &HttpHeaderMap) -> Self {
        let mut new_map = HeaderMap::with_capacity(value.inner.capacity());
        value.for_each(|name, value| {
            new_map.append(name, value.inner().clone());
        });
        new_map
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn http_header_map_operations() {
        // creation and is_empty
        let mut map = HttpHeaderMap::default();
        assert!(map.is_empty());

        // insert, contains_key, get, and is_empty after insertion
        let name1 = HeaderName::from_static("content-type");
        let value1 = HttpHeaderValue::from_static("text/plain");
        assert!(!map.contains_key(&name1));
        assert!(map.insert(name1.clone(), value1.clone()).is_none());
        assert!(map.contains_key(&name1));
        assert!(!map.is_empty());
        assert_eq!(map.get(&name1).unwrap().to_str(), "text/plain");

        // replacing a value
        let value2 = HttpHeaderValue::from_static("application/json");
        let old_value = map.insert(name1.clone(), value2).unwrap();
        assert_eq!(old_value.to_str(), "text/plain");
        assert_eq!(map.get(&name1).unwrap().to_str(), "application/json");

        // get_mut
        let mut_ref = map.get_mut(&name1).unwrap();
        mut_ref.set_static_value("text/html");
        assert_eq!(map.get(&name1).unwrap().to_str(), "text/html");

        // append and get_all
        let name2 = HeaderName::from_static("set-cookie");
        let cookie1 = HttpHeaderValue::from_static("cookie1=value1");
        let cookie2 = HttpHeaderValue::from_static("cookie2=value2");
        map.append(name2.clone(), cookie1);
        map.append(name2.clone(), cookie2);
        let all_cookies: Vec<_> = map.get_all(&name2).iter().map(|v| v.to_str()).collect();
        assert_eq!(all_cookies, vec!["cookie1=value1", "cookie2=value2"]);

        // for_each
        let mut collected_headers = HashMap::new();
        map.for_each(|name, value| {
            collected_headers
                .entry(name.to_string())
                .or_insert_with(Vec::new)
                .push(value.to_str().to_string());
        });
        assert_eq!(collected_headers.len(), 2);
        assert_eq!(
            collected_headers.get("content-type").unwrap(),
            &vec!["text/html"]
        );
        assert_eq!(
            collected_headers.get("set-cookie").unwrap(),
            &vec!["cookie1=value1", "cookie2=value2"]
        );

        // remove
        let removed_value = map.remove(&name1).unwrap();
        assert_eq!(removed_value.to_str(), "text/html");
        assert!(!map.contains_key(&name1));

        // drain
        let mut drained_map = map.clone();
        assert!(!drained_map.is_empty());
        let drained_items: Vec<_> = drained_map.drain().collect();
        assert_eq!(drained_items.len(), 2); // two set-cookie values
        assert!(drained_map.is_empty());

        // From<&HttpHeaderMap> for HeaderMap
        let mut map_for_ref_conv = HttpHeaderMap::default();
        map_for_ref_conv.insert(
            HeaderName::from_static("x-ref"),
            HttpHeaderValue::from_static("ref-value"),
        );
        let header_map_from_ref: HeaderMap = (&map_for_ref_conv).into();
        assert_eq!(header_map_from_ref.get("x-ref").unwrap(), "ref-value");

        // From<HttpHeaderMap> for HeaderMap
        let mut map_for_owned_conv = HttpHeaderMap::default();
        map_for_owned_conv.insert(
            HeaderName::from_static("x-owned"),
            HttpHeaderValue::from_static("owned-value"),
        );
        let header_map_from_owned: HeaderMap = map_for_owned_conv.into();
        assert_eq!(header_map_from_owned.get("x-owned").unwrap(), "owned-value");
    }
}
