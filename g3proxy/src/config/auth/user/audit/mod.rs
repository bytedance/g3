/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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
