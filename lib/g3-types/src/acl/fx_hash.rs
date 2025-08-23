/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::borrow::Borrow;
use std::hash::Hash;

use rustc_hash::FxHashMap;

use super::{AclAction, ActionContract};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AclFxHashRule<K, Action = AclAction>
where
    K: Hash + Eq,
{
    inner: FxHashMap<K, Action>,
    missed_action: Action,
}

impl<K, Action> AclFxHashRule<K, Action>
where
    K: Hash + Eq,
    Action: ActionContract,
{
    pub fn new(missed_action: Action) -> Self {
        AclFxHashRule {
            inner: FxHashMap::default(),
            missed_action,
        }
    }

    #[inline]
    pub fn add_node(&mut self, node: K, action: Action) {
        self.inner.insert(node, action);
    }

    #[inline]
    pub fn set_missed_action(&mut self, action: Action) {
        self.missed_action = action;
    }

    pub fn check<Q>(&self, node: &Q) -> (bool, Action)
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        if let Some(action) = self.inner.get(node) {
            (true, *action)
        } else {
            (false, self.missed_action)
        }
    }
}
