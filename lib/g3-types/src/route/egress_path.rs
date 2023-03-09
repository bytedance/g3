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

use std::str::FromStr;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum EgressPathSelection {
    #[default]
    Default,
    Index(usize),
}

impl EgressPathSelection {
    /// get the selection id
    /// `len` should not be zero
    /// the returned id will be in range 0..len
    pub fn select_by_index(&self, len: usize) -> Option<usize> {
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
}

impl FromStr for EgressPathSelection {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.eq("default") {
            return Ok(EgressPathSelection::Default);
        };

        if let Ok(index) = usize::from_str(s) {
            return Ok(EgressPathSelection::Index(index));
        }

        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

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
