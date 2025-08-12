/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use ahash::AHashMap;
use radix_trie::{Trie, TrieCommon};

#[derive(Clone, Debug, PartialEq)]
pub struct UriPathMatch<T> {
    prefix: Option<Trie<String, T>>,
    default: Option<T>,
}

impl<T> Default for UriPathMatch<T> {
    fn default() -> Self {
        UriPathMatch {
            prefix: None,
            default: None,
        }
    }
}

impl<T> UriPathMatch<T> {
    pub fn add_prefix(&mut self, prefix: String, v: T) -> Option<T> {
        self.prefix
            .get_or_insert(Default::default())
            .insert(prefix, v)
    }

    #[inline]
    pub fn set_default(&mut self, v: T) -> Option<T> {
        self.default.replace(v)
    }

    pub fn get(&self, path: &str) -> Option<&T> {
        if let Some(trie) = &self.prefix
            && let Some(v) = trie.get(path)
        {
            return Some(v);
        }

        self.default.as_ref()
    }
}

impl<'a, S, D, E> TryFrom<&'a UriPathMatch<Arc<S>>> for UriPathMatch<Arc<D>>
where
    D: TryFrom<&'a Arc<S>, Error = E>,
{
    type Error = E;

    fn try_from(src: &'a UriPathMatch<Arc<S>>) -> Result<Self, Self::Error> {
        use std::collections::hash_map::Entry;

        let mut dst = UriPathMatch::default();

        let mut tmp_ht = AHashMap::new();

        let mut get_tmp = |v| {
            let v_index = Arc::as_ptr(v) as usize;
            let dv = match tmp_ht.entry(v_index) {
                Entry::Occupied(oe) => Arc::clone(oe.get()),
                Entry::Vacant(ve) => {
                    let dv = D::try_from(v)?;
                    let dv = Arc::new(dv);
                    ve.insert(dv.clone());
                    dv
                }
            };
            Ok(dv)
        };

        if let Some(trie) = &src.prefix {
            let mut dst_trie = Trie::new();
            for (prefix, v) in trie.iter() {
                let dv = get_tmp(v)?;
                dst_trie.insert(prefix.to_string(), dv);
            }
            dst.prefix = Some(dst_trie);
        }

        if let Some(default) = &src.default {
            let v_index = Arc::as_ptr(default) as usize;
            if let Some(dv) = tmp_ht.get(&v_index) {
                dst.default = Some(Arc::clone(dv));
            } else {
                let dv = D::try_from(default)?;
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
        // A default UriPathMatch is empty and returns None
        let m: UriPathMatch<i32> = UriPathMatch::default();
        assert!(m.prefix.is_none());
        assert!(m.default.is_none());
        assert_eq!(m.get("/any/path"), None);
    }

    #[test]
    fn add_prefix() {
        // Adding new and replacing existing prefixes
        let mut m = UriPathMatch::default();
        let key = "/api/v1/".to_string();

        assert_eq!(m.add_prefix(key.clone(), 1), None);
        assert!(m.prefix.is_some());
        assert_eq!(m.get(&key), Some(&1));

        assert_eq!(m.add_prefix(key.clone(), 2), Some(1));
        assert_eq!(m.get(&key), Some(&2));

        assert_eq!(m.get("/api/"), None);
    }

    #[test]
    fn set_default() {
        // Setting and replacing the default value
        let mut m = UriPathMatch::default();

        assert_eq!(m.set_default(100), None);
        assert_eq!(m.default, Some(100));
        assert_eq!(m.get("/a/path/that/does/not/match"), Some(&100));

        assert_eq!(m.set_default(200), Some(100));
        assert_eq!(m.get("/another/path"), Some(&200));
    }

    #[test]
    fn get_exact_or_default() {
        // The get() method's behavior: exact match or fall back to default
        let mut m = UriPathMatch::default();
        m.add_prefix("/api/v1".to_string(), 1);
        m.add_prefix("/api/v2".to_string(), 2);
        m.set_default(0);

        assert_eq!(m.get("/api/v1"), Some(&1));
        assert_eq!(m.get("/api/v2"), Some(&2));

        assert_eq!(m.get("/api"), Some(&0));
        assert_eq!(m.get("/api/v1/users"), Some(&0));
        assert_eq!(m.get("/home"), Some(&0));
    }

    // Helper structs for TryFrom tests
    #[derive(Debug, PartialEq, Eq)]
    struct Src(i32);
    #[derive(Debug, PartialEq, Eq)]
    struct Dst(u32);

    // A TryFrom implementation that can fail
    impl<'a> TryFrom<&'a Arc<Src>> for Dst {
        type Error = &'static str;

        fn try_from(value: &'a Arc<Src>) -> Result<Self, Self::Error> {
            if value.0 < 0 {
                Err("Cannot convert negative value")
            } else {
                Ok(Dst(value.0 as u32))
            }
        }
    }

    #[test]
    fn try_from_success() {
        // Successful conversion using TryFrom
        let mut src: UriPathMatch<Arc<Src>> = UriPathMatch::default();
        src.add_prefix("/a".to_string(), Arc::new(Src(1)));
        src.set_default(Arc::new(Src(100)));

        let dst: UriPathMatch<Arc<Dst>> = UriPathMatch::try_from(&src).unwrap();

        assert_eq!(dst.get("/a"), Some(&Arc::new(Dst(1))));
        assert_eq!(dst.get("/c"), Some(&Arc::new(Dst(100))));
    }

    #[test]
    fn try_from_reuse_arc() {
        // Conversion reuses the created Arc for identical source Arcs
        let mut src: UriPathMatch<Arc<Src>> = UriPathMatch::default();
        let shared_val = Arc::new(Src(50));

        src.add_prefix("/a".to_string(), Arc::clone(&shared_val));
        src.add_prefix("/b".to_string(), Arc::clone(&shared_val));
        src.set_default(Arc::clone(&shared_val));

        let dst: UriPathMatch<Arc<Dst>> = UriPathMatch::try_from(&src).unwrap();

        let v_a = dst.get("/a").unwrap();
        let v_b = dst.get("/b").unwrap();
        let v_default = dst.get("/d").unwrap();

        assert!(Arc::ptr_eq(v_a, v_b));
        assert!(Arc::ptr_eq(v_a, v_default));
    }

    #[test]
    fn try_from_empty() {
        // Converting an empty UriPathMatch
        let src: UriPathMatch<Arc<Src>> = UriPathMatch::default();
        let dst: UriPathMatch<Arc<Dst>> = UriPathMatch::try_from(&src).unwrap();
        assert_eq!(dst, UriPathMatch::default());
    }

    #[test]
    fn try_from_error_in_prefix() {
        // Conversion from a value in the prefix trie that fails
        let mut src: UriPathMatch<Arc<Src>> = UriPathMatch::default();
        src.add_prefix("/valid".to_string(), Arc::new(Src(1)));
        src.add_prefix("/invalid".to_string(), Arc::new(Src(-1)));

        let result: Result<UriPathMatch<Arc<Dst>>, _> = UriPathMatch::try_from(&src);
        assert!(result.is_err());
    }

    #[test]
    fn try_from_error_in_default() {
        // Conversion from the default value that fails
        let mut src: UriPathMatch<Arc<Src>> = UriPathMatch::default();
        src.set_default(Arc::new(Src(-100)));

        let result: Result<UriPathMatch<Arc<Dst>>, _> = UriPathMatch::try_from(&src);
        assert!(result.is_err());
    }
}
