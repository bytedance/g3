/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use foldhash::HashMap;

use g3_types::metrics::NodeName;
use g3_types::net::UpstreamAddr;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct EgressPathSelection {
    number: HashMap<NodeName, usize>,
    string: HashMap<NodeName, String>,
    upstream: HashMap<NodeName, UpstreamAddr>,
    json: HashMap<NodeName, serde_json::Value>,
}

impl EgressPathSelection {
    pub(crate) fn is_empty(&self) -> bool {
        self.number.is_empty() && self.string.is_empty() && self.json.is_empty()
    }

    pub(crate) fn set_number_id(&mut self, escaper: NodeName, id: usize) {
        self.number.insert(escaper, id);
    }

    /// get the selection id
    /// `len` should not be zero
    /// the returned id will be in range 0..len
    pub(crate) fn select_number_id(&self, escaper: &NodeName, len: usize) -> Option<usize> {
        let id = self.number.get(escaper)?;
        let id = *id;
        let i = if id == 0 {
            len - 1
        } else if id <= len {
            id - 1
        } else {
            (id - 1) % len
        };
        Some(i)
    }

    pub(crate) fn set_string_id(&mut self, escaper: NodeName, id: String) {
        self.string.insert(escaper, id);
    }

    pub(crate) fn select_string_id(&self, escaper: &NodeName) -> Option<&str> {
        self.string.get(escaper).map(|s| s.as_str())
    }

    pub(crate) fn set_upstream(&mut self, escaper: NodeName, ups: UpstreamAddr) {
        self.upstream.insert(escaper, ups);
    }

    pub(crate) fn select_upstream(&self, escaper: &NodeName) -> Option<&UpstreamAddr> {
        self.upstream.get(escaper)
    }

    pub(crate) fn set_json_value(&mut self, escaper: NodeName, v: serde_json::Value) {
        self.json.insert(escaper, v);
    }

    pub(crate) fn select_json_value(&self, escaper: &NodeName) -> Option<&serde_json::Value> {
        self.json.get(escaper)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_index() {
        const LENGTH: usize = 30;
        const ESCAPER: NodeName = NodeName::new_static("abcd");

        let mut egress_path = EgressPathSelection::default();
        egress_path.set_number_id(ESCAPER.clone(), 1);
        assert_eq!(Some(0), egress_path.select_number_id(&ESCAPER, LENGTH));

        egress_path.set_number_id(ESCAPER.clone(), 2);
        assert_eq!(Some(1), egress_path.select_number_id(&ESCAPER, LENGTH));

        egress_path.set_number_id(ESCAPER.clone(), 30);
        assert_eq!(Some(29), egress_path.select_number_id(&ESCAPER, LENGTH));

        egress_path.set_number_id(ESCAPER.clone(), 0);
        assert_eq!(Some(29), egress_path.select_number_id(&ESCAPER, LENGTH));

        egress_path.set_number_id(ESCAPER.clone(), 31);
        assert_eq!(Some(0), egress_path.select_number_id(&ESCAPER, LENGTH));

        egress_path.set_number_id(ESCAPER.clone(), 60);
        assert_eq!(Some(29), egress_path.select_number_id(&ESCAPER, LENGTH));

        egress_path.set_number_id(ESCAPER.clone(), 61);
        assert_eq!(Some(0), egress_path.select_number_id(&ESCAPER, LENGTH));
    }
}
