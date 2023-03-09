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
        if let Some(trie) = &self.prefix {
            if let Some(v) = trie.get(path) {
                return Some(v);
            }
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
