/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use ahash::AHashMap;
use indexmap::IndexSet;

#[derive(Clone, Debug, PartialEq)]
pub struct AlpnMatch<T> {
    all_protocols: IndexSet<String>,
    full_match: Option<AHashMap<String, T>>,
    main_match: Option<AHashMap<String, T>>,
    default: Option<T>,
}

impl<T> Default for AlpnMatch<T> {
    fn default() -> Self {
        AlpnMatch {
            all_protocols: IndexSet::new(),
            full_match: None,
            main_match: None,
            default: None,
        }
    }
}

impl<T> AlpnMatch<T> {
    pub fn add_protocol(&mut self, protocol: String, v: T) -> Option<T> {
        self.all_protocols.insert(protocol.clone());
        if protocol.contains('/') {
            self.full_match
                .get_or_insert(Default::default())
                .insert(protocol, v)
        } else {
            self.main_match
                .get_or_insert(Default::default())
                .insert(protocol, v)
        }
    }

    #[inline]
    pub fn set_default(&mut self, v: T) -> Option<T> {
        self.default.replace(v)
    }

    pub fn get(&self, protocol: &str) -> Option<&T> {
        if let Some(p) = memchr::memchr(b'/', protocol.as_bytes()) {
            if let Some(ht) = &self.full_match
                && let Some(v) = ht.get(protocol)
            {
                return Some(v);
            }

            if let Some(ht) = &self.main_match
                && let Some(v) = ht.get(&protocol[0..p])
            {
                return Some(v);
            }
        } else if let Some(ht) = &self.main_match
            && let Some(v) = ht.get(protocol)
        {
            return Some(v);
        }

        self.default.as_ref()
    }

    #[inline]
    pub fn get_default(&self) -> Option<&T> {
        self.default.as_ref()
    }

    pub fn is_empty(&self) -> bool {
        self.full_match.is_none() && self.main_match.is_none() && self.default.is_none()
    }

    #[inline]
    pub fn protocols(&self) -> &IndexSet<String> {
        &self.all_protocols
    }

    pub fn build<R, F>(&self, find: F) -> AlpnMatch<R>
    where
        F: Fn(&T) -> R,
    {
        let mut dst = AlpnMatch {
            all_protocols: self.all_protocols.clone(),
            ..Default::default()
        };

        if let Some(ht) = &self.full_match {
            let mut dst_ht = AHashMap::with_capacity(ht.len());
            for (k, name) in ht {
                let dv = find(name);
                dst_ht.insert(k.to_string(), dv);
            }
            dst.full_match = Some(dst_ht);
        }

        if let Some(ht) = &self.main_match {
            let mut dst_ht = AHashMap::with_capacity(ht.len());
            for (k, name) in ht {
                let dv = find(name);
                dst_ht.insert(k.to_string(), dv);
            }
            dst.main_match = Some(dst_ht);
        }

        if let Some(default) = &self.default {
            let dv = find(default);
            dst.default = Some(dv);
        }

        dst
    }
}

impl<T: PartialEq> AlpnMatch<T> {
    pub fn contains_value(&self, value: &T) -> bool {
        if let Some(ht) = &self.full_match {
            for v in ht.values() {
                if v.eq(value) {
                    return true;
                }
            }
        }

        if let Some(ht) = &self.main_match {
            for v in ht.values() {
                if v.eq(value) {
                    return true;
                }
            }
        }

        if let Some(v) = &self.default
            && v.eq(value)
        {
            return true;
        }

        false
    }
}

impl<T> AlpnMatch<Arc<T>> {
    pub fn try_build_arc<R, E, F>(&self, try_find: F) -> Result<AlpnMatch<Arc<R>>, E>
    where
        F: Fn(&Arc<T>) -> Result<R, E>,
    {
        use std::collections::hash_map::Entry;

        let mut dst = AlpnMatch {
            all_protocols: self.all_protocols.clone(),
            ..Default::default()
        };

        let mut tmp_ht = AHashMap::new();

        let mut get_tmp = |v| {
            let v_index = Arc::as_ptr(v) as usize;
            let dv = match tmp_ht.entry(v_index) {
                Entry::Occupied(oe) => Arc::clone(oe.get()),
                Entry::Vacant(ve) => {
                    let dv = try_find(v)?;
                    let dv = Arc::new(dv);
                    ve.insert(dv.clone());
                    dv
                }
            };
            Ok(dv)
        };

        if let Some(ht) = &self.full_match {
            let mut dst_ht = AHashMap::with_capacity(ht.len());
            for (k, v) in ht {
                let dv = get_tmp(v)?;
                dst_ht.insert(k.to_string(), dv);
            }
            dst.full_match = Some(dst_ht);
        }

        if let Some(ht) = &self.main_match {
            let mut dst_ht = AHashMap::with_capacity(ht.len());
            for (k, v) in ht {
                let dv = get_tmp(v)?;
                dst_ht.insert(k.to_string(), dv);
            }
            dst.main_match = Some(dst_ht);
        }

        if let Some(default) = &self.default {
            let v_index = Arc::as_ptr(default) as usize;
            if let Some(dv) = tmp_ht.get(&v_index) {
                dst.default = Some(Arc::clone(dv));
            } else {
                let dv = try_find(default)?;
                dst.default = Some(Arc::new(dv));
            }
        }

        Ok(dst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_empty() {
        // Default construction and empty state
        let alpn: AlpnMatch<i32> = AlpnMatch::default();
        assert!(alpn.is_empty());
        assert!(alpn.protocols().is_empty());
        assert!(alpn.get_default().is_none());
        assert!(alpn.get("http").is_none());
    }

    #[test]
    fn add_protocol_full_match() {
        // Adding a full protocol (with /)
        let mut alpn = AlpnMatch::default();
        assert_eq!(alpn.add_protocol("http/1.1".to_string(), 1), None);
        assert_eq!(alpn.protocols().len(), 1);
        assert!(alpn.protocols().contains("http/1.1"));
        assert_eq!(alpn.get("http/1.1"), Some(&1));
        assert_eq!(alpn.get("http"), None);
        assert!(!alpn.is_empty());
    }

    #[test]
    fn add_protocol_main_match() {
        // Adding a main protocol (no /)
        let mut alpn = AlpnMatch::default();
        assert_eq!(alpn.add_protocol("http".to_string(), 2), None);
        assert_eq!(alpn.protocols().len(), 1);
        assert!(alpn.protocols().contains("http"));
        assert_eq!(alpn.get("http"), Some(&2));
        assert_eq!(alpn.get("http/1.1"), Some(&2));
        assert!(!alpn.is_empty());
    }

    #[test]
    fn add_protocol_replace() {
        // Replacing existing protocol values
        let mut alpn = AlpnMatch::default();
        assert_eq!(alpn.add_protocol("http/1.1".to_string(), 1), None);
        assert_eq!(alpn.add_protocol("http/1.1".to_string(), 3), Some(1));
        assert_eq!(alpn.get("http/1.1"), Some(&3));
    }

    #[test]
    fn set_default() {
        // Setting and replacing default value
        let mut alpn = AlpnMatch::default();
        assert_eq!(alpn.set_default(0), None);
        assert_eq!(alpn.get_default(), Some(&0));
        assert_eq!(alpn.get("unknown"), Some(&0));
        assert_eq!(alpn.set_default(5), Some(0));
        assert_eq!(alpn.get_default(), Some(&5));
        assert!(!alpn.is_empty());
    }

    #[test]
    fn get_precedence() {
        // Precedence: full match > main match > default
        let mut alpn = AlpnMatch::default();
        alpn.add_protocol("http/1.1".to_string(), 1);
        alpn.add_protocol("http".to_string(), 2);
        alpn.set_default(0);
        assert_eq!(alpn.get("http/1.1"), Some(&1)); // Full match
        assert_eq!(alpn.get("http/2"), Some(&2)); // Main match
        assert_eq!(alpn.get("unknown"), Some(&0)); // Default
    }

    #[test]
    fn contains_value() {
        // Contains_value with PartialEq
        let mut alpn = AlpnMatch::default();
        alpn.add_protocol("http/1.1".to_string(), 1);
        alpn.add_protocol("http".to_string(), 2);
        alpn.set_default(0);
        assert!(alpn.contains_value(&1));
        assert!(alpn.contains_value(&2));
        assert!(alpn.contains_value(&0));
        assert!(!alpn.contains_value(&3));
    }

    #[test]
    fn build() {
        // Build method transforming values
        let mut alpn = AlpnMatch::default();
        alpn.add_protocol("http/1.1".to_string(), 1);
        alpn.add_protocol("http".to_string(), 2);
        alpn.set_default(0);
        let transformed = alpn.build(|x| x.to_string());
        assert_eq!(transformed.get("http/1.1"), Some(&"1".to_string()));
        assert_eq!(transformed.get("http/2"), Some(&"2".to_string()));
        assert_eq!(transformed.get("unknown"), Some(&"0".to_string()));
        assert_eq!(transformed.protocols(), alpn.protocols());
    }

    #[test]
    fn try_build_arc_success() {
        // Try_build_arc with successful transformation
        let mut alpn = AlpnMatch::default();
        let v1 = Arc::new(1);
        let v2 = Arc::new(2);
        let v0 = Arc::new(0);
        alpn.add_protocol("http/1.1".to_string(), Arc::clone(&v1));
        alpn.add_protocol("http".to_string(), Arc::clone(&v2));
        alpn.set_default(Arc::clone(&v0));
        let transformed = alpn.try_build_arc(|x| Ok::<_, ()>(x.to_string())).unwrap();
        assert_eq!(
            transformed.get("http/1.1"),
            Some(&Arc::new("1".to_string()))
        );
        assert_eq!(transformed.get("http/2"), Some(&Arc::new("2".to_string())));
        assert_eq!(transformed.get("unknown"), Some(&Arc::new("0".to_string())));
        assert_eq!(transformed.protocols(), alpn.protocols());
    }

    #[test]
    fn try_build_arc_reuse() {
        // Try_build_arc reuses Arc values for identical pointers
        let mut alpn = AlpnMatch::default();
        let v1 = Arc::new(1);
        alpn.add_protocol("http/1.1".to_string(), Arc::clone(&v1));
        alpn.add_protocol("http/2".to_string(), Arc::clone(&v1)); // Same Arc
        let transformed = alpn.try_build_arc(|x| Ok::<_, ()>(x.to_string())).unwrap();
        let val1 = transformed.get("http/1.1").unwrap();
        let val2 = transformed.get("http/2").unwrap();
        assert!(Arc::ptr_eq(val1, val2)); // Same Arc due to caching
    }

    #[test]
    fn try_build_arc_error() {
        // Try_build_arc with error case
        let mut alpn = AlpnMatch::default();
        alpn.add_protocol("http/1.1".to_string(), Arc::new(1));
        let result = alpn.try_build_arc(|_| Err::<String, _>(()));
        assert!(result.is_err());
    }
}
