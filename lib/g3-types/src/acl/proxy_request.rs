/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::{AclAction, AclFxHashRule, ActionContract};
use crate::net::ProxyRequestType;

#[derive(Clone)]
pub struct AclProxyRequestRule<Action = AclAction> {
    missed_action: Action,
    request: AclFxHashRule<ProxyRequestType, Action>,
}

impl<Action: ActionContract> AclProxyRequestRule<Action> {
    #[inline]
    pub fn new(missed_action: Action) -> Self {
        AclProxyRequestRule {
            missed_action,
            request: AclFxHashRule::new(missed_action),
        }
    }

    #[inline]
    pub fn add_request_type(&mut self, request: ProxyRequestType, action: Action) {
        self.request.add_node(request, action);
    }

    #[inline]
    pub fn set_missed_action(&mut self, action: Action) {
        self.missed_action = action;
        self.request.set_missed_action(action);
    }

    #[inline]
    pub fn missed_action(&self) -> Action {
        self.missed_action
    }

    #[inline]
    pub fn check_request(&self, request: &ProxyRequestType) -> (bool, Action) {
        self.request.check(request)
    }
}
