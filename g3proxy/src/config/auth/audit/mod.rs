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

use rand::distributions::{Bernoulli, Distribution};

mod json;
mod yaml;

#[derive(Default, Debug, Clone, PartialEq)]
pub(crate) struct UserAuditConfig {
    pub(crate) enable_protocol_inspection: bool,
    pub(crate) prohibit_unknown_protocol: bool,
    application_audit_ratio: Option<Bernoulli>,
}

impl UserAuditConfig {
    pub(crate) fn do_application_audit(&self) -> Option<bool> {
        if let Some(ratio) = &self.application_audit_ratio {
            let mut rng = rand::thread_rng();
            Some(ratio.sample(&mut rng))
        } else {
            None
        }
    }
}
