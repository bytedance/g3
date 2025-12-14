/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::hash::Hash;
use std::net::IpAddr;
use std::sync::Arc;

use ahash::AHashMap;
use arcstr::ArcStr;
use radix_trie::{Trie, TrieCommon};
use rustc_hash::{FxBuildHasher, FxHashMap};

use crate::collection::NamedValue;
use crate::net::Host;
use crate::resolve::reverse_idna_domain;

#[derive(Clone, Debug, PartialEq)]
pub struct HostMatch<T> {
    exact_domain: Option<AHashMap<ArcStr, T>>,
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
    pub fn add_exact_domain(&mut self, domain: ArcStr, v: T) -> Option<T> {
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
                if let Some(ht) = &self.exact_ip
                    && let Some(v) = ht.get(ip)
                {
                    return Some(v);
                }
            }
            Host::Domain(domain) => {
                if let Some(ht) = &self.exact_domain
                    && let Some(v) = ht.get(domain)
                {
                    return Some(v);
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

        if let Some(default) = &self.default
            && let Some(dv) = values.get(default.name())
        {
            dst.default = Some(dv.clone());
        }

        dst
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    // Helper struct for NamedValue impl
    #[derive(Debug, PartialEq, Eq)]
    struct TestValue(&'static str);

    impl NamedValue for TestValue {
        type Name = str;
        type NameOwned = String;

        fn name(&self) -> &str {
            self.0
        }
        fn name_owned(&self) -> String {
            self.0.to_string()
        }
    }

    // Helper struct for try_build_arc tests
    #[derive(Debug)]
    struct Src(i32);
    #[derive(Debug, PartialEq)]
    struct Dst(i32);

    #[test]
    fn basic_operations() {
        let mut hm = HostMatch::default();
        assert!(hm.is_empty());

        assert_eq!(
            hm.add_exact_domain(arcstr::literal!("example.com"), 1),
            None
        );
        assert_eq!(
            hm.add_exact_ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 2),
            None
        );
        assert_eq!(hm.add_child_domain("test.com", 3), None);

        assert_eq!(hm.set_default(4), None);
        assert!(!hm.is_empty());

        assert_eq!(
            hm.add_exact_domain(arcstr::literal!("example.com"), 5),
            Some(1)
        );
    }

    #[test]
    fn get_matching() {
        let mut hm = HostMatch::default();
        hm.add_exact_domain(arcstr::literal!("example.com"), 1);
        hm.add_exact_ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 2);
        hm.add_child_domain("sub.test.com", 3);
        hm.set_default(4);

        assert_eq!(
            hm.get(&Host::Domain(arcstr::literal!("example.com"))),
            Some(&1)
        );

        assert_eq!(
            hm.get(&Host::Ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)))),
            Some(&2)
        );

        assert_eq!(
            hm.get(&Host::Domain(arcstr::literal!("a.sub.test.com"))),
            Some(&4)
        );

        assert_eq!(
            hm.get(&Host::Domain(arcstr::literal!("unknown.com"))),
            Some(&4)
        );
        assert_eq!(
            hm.get(&Host::Ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)))),
            Some(&4)
        );
    }

    #[test]
    fn try_build_arc_success() {
        let mut hm = HostMatch::default();
        hm.add_exact_ip(IpAddr::V4(Ipv4Addr::LOCALHOST), Arc::new(Src(10)));
        hm.set_default(Arc::new(Src(20)));

        let result = hm.try_build_arc(|src| Ok::<_, &str>(Dst(src.0 * 2)));
        assert!(result.is_ok());
        let hm_dst = result.unwrap();

        assert_eq!(
            hm_dst.get(&Host::Ip(IpAddr::V4(Ipv4Addr::LOCALHOST))),
            Some(&Arc::new(Dst(20)))
        );
        assert_eq!(hm_dst.get_default(), Some(&Arc::new(Dst(40))));
    }

    #[test]
    fn try_build_arc_error() {
        let mut hm = HostMatch::default();
        hm.add_exact_domain(arcstr::literal!("error.com"), Arc::new(Src(-1)));

        let result = hm.try_build_arc(|src| {
            if src.0 < 0 {
                Err("Negative value")
            } else {
                Ok(Dst(src.0))
            }
        });

        assert!(result.is_err());
    }

    #[test]
    fn try_build_arc_reuse() {
        let shared = Arc::new(Src(100));
        let mut hm = HostMatch::default();
        hm.add_exact_domain(arcstr::literal!("a.com"), Arc::clone(&shared));
        hm.add_exact_domain(arcstr::literal!("b.com"), Arc::clone(&shared));

        let hm_dst = hm.try_build_arc(|src| Ok::<_, ()>(Dst(src.0))).unwrap();
        let a_val = hm_dst
            .get(&Host::Domain(arcstr::literal!("a.com")))
            .unwrap();
        let b_val = hm_dst
            .get(&Host::Domain(arcstr::literal!("b.com")))
            .unwrap();

        assert!(Arc::ptr_eq(a_val, b_val));
    }

    #[test]
    fn get_all_values() {
        let mut hm = HostMatch::<Arc<TestValue>>::default();
        hm.add_exact_domain(arcstr::literal!("a.com"), Arc::new(TestValue("a")));
        hm.add_exact_ip(IpAddr::V4(Ipv4Addr::LOCALHOST), Arc::new(TestValue("b")));
        hm.add_child_domain("c.com", Arc::new(TestValue("c")));
        hm.set_default(Arc::new(TestValue("d")));

        let values = hm.get_all_values();
        assert_eq!(values.len(), 4);
        assert_eq!(values.get("a").unwrap().0, "a");
        assert_eq!(values.get("b").unwrap().0, "b");
        assert_eq!(values.get("c").unwrap().0, "c");
        assert_eq!(values.get("d").unwrap().0, "d");
    }

    #[test]
    fn build_from() {
        let mut hm_src = HostMatch::<Arc<TestValue>>::default();
        hm_src.add_exact_domain(arcstr::literal!("a.com"), Arc::new(TestValue("a")));
        hm_src.set_default(Arc::new(TestValue("default")));

        let mut values = AHashMap::new();
        values.insert("a".to_string(), Arc::new("mapped_a"));
        values.insert("default".to_string(), Arc::new("mapped_default"));

        let hm_dst = hm_src.build_from(values);

        assert_eq!(
            hm_dst.get(&Host::Domain(arcstr::literal!("a.com"))),
            Some(&Arc::from("mapped_a"))
        );
        assert_eq!(
            hm_dst.get(&Host::Domain(arcstr::literal!("unknown.com"))),
            Some(&Arc::from("mapped_default"))
        );
    }
}
