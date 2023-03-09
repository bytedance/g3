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

use std::collections::{BTreeMap, BTreeSet};

use anyhow::anyhow;

pub(super) struct EscaperConfigVerifier {}

impl EscaperConfigVerifier {
    pub(super) fn check_duplicated_rule<T>(
        input_map: &BTreeMap<String, BTreeSet<T>>,
    ) -> anyhow::Result<()>
    where
        T: std::fmt::Display,
    {
        let mut table = BTreeMap::<String, String>::new();
        for (escaper, set) in input_map {
            for entry in set {
                if let Some(old_escaper) = table.insert(entry.to_string(), escaper.to_string()) {
                    return Err(anyhow!(
                        "rule {entry} is added both for escaper {escaper} and {old_escaper}"
                    ));
                }
            }
        }
        Ok(())
    }
}
