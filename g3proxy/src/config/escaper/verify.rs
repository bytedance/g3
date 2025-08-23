/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use anyhow::anyhow;

use g3_types::metrics::NodeName;

pub(super) struct EscaperConfigVerifier {}

impl EscaperConfigVerifier {
    pub(super) fn check_duplicated_rule<T>(
        input_map: &BTreeMap<NodeName, BTreeSet<T>>,
    ) -> anyhow::Result<()>
    where
        T: fmt::Display,
    {
        let mut table = BTreeMap::<String, NodeName>::new();
        for (escaper, set) in input_map {
            for entry in set {
                if let Some(old_escaper) = table.insert(entry.to_string(), escaper.clone()) {
                    return Err(anyhow!(
                        "rule {entry} is added both for escaper {escaper} and {old_escaper}"
                    ));
                }
            }
        }
        Ok(())
    }
}
