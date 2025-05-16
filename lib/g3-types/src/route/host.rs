/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::hash::Hash;
use std::net::IpAddr;
use std::sync::Arc;

use ahash::AHashMap;
use radix_trie::{Trie, TrieCommon};
use rustc_hash::{FxBuildHasher, FxHashMap};

use crate::collection::NamedValue;
use crate::net::Host;
use crate::resolve::reverse_idna_domain;

#[derive(Clone, Debug, PartialEq)]
pub struct HostMatch<T> {
    exact_domain: Option<AHashMap<Arc<str>, T>>,
    exact_ip: Option<FxHashMap<IpAddr, T>>,
    child_domain: Option<Trie<String, T>>,
    default: Option<T>,
}

impl<T> Default for HostMatch<T> {
    fn default() -> Self {
        HostMatch {
            exact_domain: None,
            exact_ip: None,
            child_domain: None,
            default: None,
        }
    }
}

impl<T> HostMatch<T> {
    pub fn add_exact_domain(&mut self, domain: Arc<str>, v: T) -> Option<T> {
        self.exact_domain
            .get_or_insert(Default::default())
            .insert(domain, v)
    }

    pub fn add_exact_ip(&mut self, ip: IpAddr, v: T) -> Option<T> {
        self.exact_ip
            .get_or_insert(Default::default())
            .insert(ip, v)
    }

    pub fn add_child_domain(&mut self, domain: &str, v: T) -> Option<T> {
        self.child_domain
            .get_or_insert(Default::default())
            .insert(reverse_idna_domain(domain), v)
    }

    #[inline]
    pub fn set_default(&mut self, v: T) -> Option<T> {
        self.default.replace(v)
    }

    pub fn get(&self, host: &Host) -> Option<&T> {
        match host {
            Host::Ip(ip) => {
                if let Some(ht) = &self.exact_ip {
                    if let Some(v) = ht.get(ip) {
                        return Some(v);
                    }
                }
            }
            Host::Domain(domain) => {
                if let Some(ht) = &self.exact_domain {
                    if let Some(v) = ht.get(domain) {
                        return Some(v);
                    }
                }

                if let Some(trie) = &self.child_domain {
                    let reversed = reverse_idna_domain(domain);
                    if let Some(v) = trie.get(&reversed) {
                        return Some(v);
                    }
                }
            }
        }
        self.default.as_ref()
    }

    #[inline]
    pub fn get_default(&self) -> Option<&T> {
        self.default.as_ref()
    }

    pub fn is_empty(&self) -> bool {
        self.exact_domain.is_none()
            && self.exact_ip.is_none()
            && self.child_domain.is_none()
            && self.default.is_none()
    }
}

impl<T> HostMatch<Arc<T>> {
    pub fn try_build_arc<R, E, F>(&self, try_build: F) -> Result<HostMatch<Arc<R>>, E>
    where
        F: Fn(&Arc<T>) -> Result<R, E>,
    {
        use std::collections::hash_map::Entry;

        let mut dst = HostMatch::default();

        let mut tmp_ht = AHashMap::new();

        let mut get_tmp = |v| {
            let v_index = Arc::as_ptr(v) as usize;
            let dv = match tmp_ht.entry(v_index) {
                Entry::Occupied(oe) => Arc::clone(oe.get()),
                Entry::Vacant(ve) => {
                    let dv = try_build(v)?;
                    let dv = Arc::new(dv);
                    ve.insert(dv.clone());
                    dv
                }
            };
            Ok(dv)
        };

        if let Some(ht) = &self.exact_domain {
            let mut dst_ht = AHashMap::with_capacity(ht.len());
            for (k, v) in ht {
                let dv = get_tmp(v)?;
                dst_ht.insert(k.clone(), dv);
            }
            dst.exact_domain = Some(dst_ht);
        }

        if let Some(ht) = &self.exact_ip {
            let mut dst_ht = FxHashMap::with_capacity_and_hasher(ht.len(), FxBuildHasher);
            for (k, v) in ht {
                let dv = get_tmp(v)?;
                dst_ht.insert(*k, dv);
            }
            dst.exact_ip = Some(dst_ht);
        }

        if let Some(trie) = &self.child_domain {
            let mut dst_trie = Trie::new();
            for (prefix, v) in trie.iter() {
                let dv = get_tmp(v)?;
                dst_trie.insert(prefix.to_string(), dv);
            }
            dst.child_domain = Some(dst_trie);
        }

        if let Some(default) = &self.default {
            let v_index = Arc::as_ptr(default) as usize;
            if let Some(dv) = tmp_ht.get(&v_index) {
                dst.default = Some(Arc::clone(dv));
            } else {
                let dv = try_build(default)?;
                dst.default = Some(Arc::new(dv));
            }
        }

        Ok(dst)
    }
}

impl<T> HostMatch<Arc<T>>
where
    T: NamedValue,
    <T as NamedValue>::Name: Hash + Eq,
    <T as NamedValue>::NameOwned: Hash + Eq,
{
    pub fn get_all_values(&self) -> AHashMap<<T as NamedValue>::NameOwned, Arc<T>> {
        let mut map = AHashMap::new();

        let mut add_to_map = |v: &Arc<T>| {
            let name = v.name_owned();
            map.entry(name).or_insert_with(|| v.clone());
        };

        if let Some(ht) = &self.exact_domain {
            ht.values().for_each(&mut add_to_map);
        }

        if let Some(ht) = &self.exact_ip {
            ht.values().for_each(&mut add_to_map);
        }

        if let Some(trie) = &self.child_domain {
            trie.values().for_each(&mut add_to_map);
        }

        if let Some(default) = &self.default {
            add_to_map(default);
        }

        map
    }

    pub fn build_from<D>(
        &self,
        values: AHashMap<<T as NamedValue>::NameOwned, Arc<D>>,
    ) -> HostMatch<Arc<D>> {
        let mut dst = HostMatch::default();

        if let Some(ht) = &self.exact_domain {
            let mut dst_ht = AHashMap::with_capacity(ht.len());
            for (k, v) in ht {
                if let Some(dv) = values.get(v.name()) {
                    dst_ht.insert(k.clone(), dv.clone());
                }
            }
            dst.exact_domain = Some(dst_ht);
        }

        if let Some(ht) = &self.exact_ip {
            let mut dst_ht = FxHashMap::with_capacity_and_hasher(ht.len(), FxBuildHasher);
            for (k, v) in ht {
                if let Some(dv) = values.get(v.name()) {
                    dst_ht.insert(*k, dv.clone());
                }
            }
            dst.exact_ip = Some(dst_ht);
        }

        if let Some(trie) = &self.child_domain {
            let mut dst_trie = Trie::new();
            for (prefix, v) in trie.iter() {
                if let Some(dv) = values.get(v.name()) {
                    dst_trie.insert(prefix.to_string(), dv.clone());
                }
            }
            dst.child_domain = Some(dst_trie);
        }

        if let Some(default) = &self.default {
            if let Some(dv) = values.get(default.name()) {
                dst.default = Some(dv.clone());
            }
        }

        dst
    }
}
