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

use super::{AclAHashRule, AclAction};
use crate::net::ProxyRequestType;

#[derive(Clone)]
pub struct AclProxyRequestRule {
    missed_action: AclAction,
    request: AclAHashRule<ProxyRequestType>,
}

impl AclProxyRequestRule {
    #[inline]
    pub fn new(missed_action: AclAction) -> Self {
        AclProxyRequestRule {
            missed_action,
            request: AclAHashRule::new(missed_action),
        }
    }

    #[inline]
    pub fn add_request_type(&mut self, request: ProxyRequestType, action: AclAction) {
        self.request.add_node(request, action);
    }

    #[inline]
    pub fn set_missed_action(&mut self, action: AclAction) {
        self.missed_action = action;
        self.request.set_missed_action(action);
    }

    #[inline]
    pub fn missed_action(&self) -> AclAction {
        self.missed_action
    }

    #[inline]
    pub fn check_request(&self, request: &ProxyRequestType) -> (bool, AclAction) {
        self.request.check(request)
    }
}
