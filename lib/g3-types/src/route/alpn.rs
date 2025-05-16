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
            if let Some(ht) = &self.full_match {
                if let Some(v) = ht.get(protocol) {
                    return Some(v);
                }
            }

            if let Some(ht) = &self.main_match {
                if let Some(v) = ht.get(&protocol[0..p]) {
                    return Some(v);
                }
            }
        } else if let Some(ht) = &self.main_match {
            if let Some(v) = ht.get(protocol) {
                return Some(v);
            }
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

        if let Some(v) = &self.default {
            if v.eq(value) {
                return true;
            }
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
