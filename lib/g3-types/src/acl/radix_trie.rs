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

use std::borrow::Borrow;
use std::hash::Hash;

use ahash::AHashMap;
use radix_trie::{Trie, TrieKey};

use super::AclAction;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AclRadixTrieRuleBuilder<K>
where
    K: TrieKey + Hash,
{
    inner: AHashMap<K, AclAction>,
    missed_action: AclAction,
}

impl<K> AclRadixTrieRuleBuilder<K>
where
    K: TrieKey + Hash + Clone,
{
    pub fn new(missed_action: AclAction) -> Self {
        AclRadixTrieRuleBuilder {
            inner: AHashMap::new(),
            missed_action,
        }
    }

    #[inline]
    pub fn add_node(&mut self, node: K, action: AclAction) {
        self.inner.insert(node, action);
    }

    #[inline]
    pub fn set_missed_action(&mut self, action: AclAction) {
        self.missed_action = action;
    }

    #[inline]
    pub fn missed_action(&self) -> AclAction {
        self.missed_action
    }

    pub fn build(&self) -> AclRadixTrieRule<K> {
        let mut trie = Trie::new();

        for (k, v) in &self.inner {
            trie.insert(k.clone(), *v);
        }

        AclRadixTrieRule {
            inner: trie,
            missed_action: self.missed_action,
        }
    }
}

pub struct AclRadixTrieRule<K: TrieKey> {
    inner: Trie<K, AclAction>,
    missed_action: AclAction,
}

impl<K: TrieKey> AclRadixTrieRule<K> {
    pub fn check<Q>(&self, key: &Q) -> (bool, AclAction)
    where
        K: Borrow<Q>,
        Q: TrieKey,
    {
        if let Some(action) = self.inner.get_ancestor_value(key) {
            (true, *action)
        } else {
            (false, self.missed_action)
        }
    }
}
