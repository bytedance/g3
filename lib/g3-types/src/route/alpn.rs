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
}

impl<'a, S, D, E> TryFrom<&'a AlpnMatch<Arc<S>>> for AlpnMatch<Arc<D>>
where
    D: TryFrom<&'a Arc<S>, Error = E>,
{
    type Error = E;

    fn try_from(src: &'a AlpnMatch<Arc<S>>) -> Result<Self, Self::Error> {
        use std::collections::hash_map::Entry;

        let mut dst = AlpnMatch {
            all_protocols: src.all_protocols.clone(),
            ..Default::default()
        };

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

        if let Some(ht) = &src.full_match {
            let mut dst_ht = AHashMap::with_capacity(ht.len());
            for (k, v) in ht {
                let dv = get_tmp(v)?;
                dst_ht.insert(k.to_string(), dv);
            }
            dst.full_match = Some(dst_ht);
        }

        if let Some(ht) = &src.main_match {
            let mut dst_ht = AHashMap::with_capacity(ht.len());
            for (k, v) in ht {
                let dv = get_tmp(v)?;
                dst_ht.insert(k.to_string(), dv);
            }
            dst.main_match = Some(dst_ht);
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
