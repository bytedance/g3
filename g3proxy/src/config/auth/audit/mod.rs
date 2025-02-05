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

use rand::distr::{Bernoulli, Distribution};

mod json;
mod yaml;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UserAuditConfig {
    pub(crate) enable_protocol_inspection: bool,
    pub(crate) prohibit_unknown_protocol: bool,
    pub(crate) prohibit_timeout_protocol: bool,
    task_audit_ratio: Option<Bernoulli>,
}

impl Default for UserAuditConfig {
    fn default() -> Self {
        UserAuditConfig {
            enable_protocol_inspection: false,
            prohibit_unknown_protocol: false,
            prohibit_timeout_protocol: true,
            task_audit_ratio: None,
        }
    }
}

impl UserAuditConfig {
    pub(crate) fn do_task_audit(&self) -> Option<bool> {
        if let Some(ratio) = &self.task_audit_ratio {
            let mut rng = rand::rng();
            Some(ratio.sample(&mut rng))
        } else {
            None
        }
    }
}
