/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use std::collections::VecDeque;
use std::fmt::{self, Write};
use std::str::FromStr;

use g3_types::metrics::{NodeName, ParseError};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct MetricName {
    nodes: VecDeque<NodeName>,
}

impl MetricName {
    pub(crate) fn new(s: &str, delimiter: char) -> Result<Self, ParseError> {
        let mut nodes = VecDeque::new();
        for node in s.split(delimiter) {
            let node = NodeName::from_str(node)?;
            nodes.push_back(node);
        }

        Ok(MetricName { nodes })
    }

    pub(crate) fn add_prefix(&mut self, prefix: NodeName) {
        self.nodes.push_front(prefix);
    }

    pub(crate) fn display(&self, delimiter: char) -> MetricNameDisplay<'_> {
        MetricNameDisplay {
            nodes: &self.nodes,
            delimiter,
        }
    }
}

pub(crate) struct MetricNameDisplay<'a> {
    nodes: &'a VecDeque<NodeName>,
    delimiter: char,
}

impl fmt::Display for MetricNameDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut iter = self.nodes.iter();
        let Some(n) = iter.next() else {
            return Ok(());
        };
        f.write_str(n.as_str())?;
        for n in iter {
            f.write_char(self.delimiter)?;
            f.write_str(n.as_str())?;
        }
        Ok(())
    }
}
