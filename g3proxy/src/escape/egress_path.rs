/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use ahash::AHashMap;

use g3_types::metrics::NodeName;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum EgressPathSelection {
    Index(usize),
    MatchId(AHashMap<NodeName, String>),
    MatchValue(AHashMap<NodeName, serde_json::Value>),
}

impl EgressPathSelection {
    /// get the selection id
    /// `len` should not be zero
    /// the returned id will be in range 0..len
    pub(crate) fn select_by_index(&self, len: usize) -> Option<usize> {
        if let EgressPathSelection::Index(id) = self {
            let id = *id;
            let i = if id == 0 {
                len - 1
            } else if id <= len {
                id - 1
            } else {
                (id - 1) % len
            };
            Some(i)
        } else {
            None
        }
    }

    pub(crate) fn select_matched_id(&self, escaper: &str) -> Option<&str> {
        if let EgressPathSelection::MatchId(map) = self {
            map.get(escaper).map(|v| v.as_str())
        } else {
            None
        }
    }

    pub(crate) fn select_matched_value(&self, escaper: &str) -> Option<&serde_json::Value> {
        if let EgressPathSelection::MatchValue(map) = self {
            map.get(escaper)
        } else {
            None
        }
    }
}

impl FromStr for EgressPathSelection {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(index) = usize::from_str(s) {
            return Ok(EgressPathSelection::Index(index));
        }

        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str() {
        assert_eq!(
            EgressPathSelection::from_str("001").unwrap(),
            EgressPathSelection::Index(1)
        );
        assert_eq!(
            EgressPathSelection::from_str("1").unwrap(),
            EgressPathSelection::Index(1)
        );
    }

    #[test]
    fn select_index() {
        const LENGTH: usize = 30;

        assert_eq!(
            Some(0),
            EgressPathSelection::Index(1).select_by_index(LENGTH)
        );
        assert_eq!(
            Some(1),
            EgressPathSelection::Index(2).select_by_index(LENGTH)
        );
        assert_eq!(
            Some(29),
            EgressPathSelection::Index(30).select_by_index(LENGTH)
        );

        assert_eq!(
            Some(29),
            EgressPathSelection::Index(0).select_by_index(LENGTH)
        );

        assert_eq!(
            Some(0),
            EgressPathSelection::Index(31).select_by_index(LENGTH)
        );
        assert_eq!(
            Some(29),
            EgressPathSelection::Index(60).select_by_index(LENGTH)
        );

        assert_eq!(
            Some(0),
            EgressPathSelection::Index(61).select_by_index(LENGTH)
        );
    }
}
