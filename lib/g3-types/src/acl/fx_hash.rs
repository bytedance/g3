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

use rustc_hash::FxHashMap;

use super::AclAction;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AclFxHashRule<K>
where
    K: Hash + Eq,
{
    inner: FxHashMap<K, AclAction>,
    missed_action: AclAction,
}

impl<K> Default for AclFxHashRule<K>
where
    K: Hash + Eq,
{
    fn default() -> Self {
        Self::new(AclAction::Forbid)
    }
}

impl<K> AclFxHashRule<K>
where
    K: Hash + Eq,
{
    pub fn new(missed_action: AclAction) -> Self {
        AclFxHashRule {
            inner: FxHashMap::default(),
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

    pub fn check<Q: ?Sized>(&self, node: &Q) -> (bool, AclAction)
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        if let Some(action) = self.inner.get(node) {
            (true, *action)
        } else {
            (false, self.missed_action)
        }
    }
}
