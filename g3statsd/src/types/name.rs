/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::collections::VecDeque;
use std::fmt::{self, Write};
use std::str::FromStr;

use anyhow::anyhow;
use yaml_rust::Yaml;

use g3_types::metrics::{NodeName, ParseError};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct MetricName {
    nodes: VecDeque<NodeName>,
}

impl MetricName {
    pub(crate) fn parse(s: &str) -> Result<Self, ParseError> {
        Self::parse_with_delimiter(s, '.')
    }

    pub(crate) fn parse_with_delimiter(s: &str, delimiter: char) -> Result<Self, ParseError> {
        let mut nodes = VecDeque::new();
        for node in s.split(delimiter) {
            let node = NodeName::from_str(node)?;
            nodes.push_back(node);
        }

        Ok(MetricName { nodes })
    }

    pub(crate) fn add_prefix(&mut self, prefix: &MetricName) {
        let mut new_nodes = prefix.nodes.clone();
        new_nodes.append(&mut self.nodes);
        self.nodes = new_nodes;
    }

    pub(crate) fn display(&self, delimiter: char) -> MetricNameDisplay<'_> {
        MetricNameDisplay {
            nodes: &self.nodes,
            delimiter,
        }
    }

    pub(crate) fn parse_yaml(value: &Yaml) -> anyhow::Result<Self> {
        match value {
            Yaml::Array(_) => {
                let nodes = g3_yaml::value::as_list(value, g3_yaml::value::as_metric_node_name)?;
                Ok(MetricName::from(nodes))
            }
            Yaml::String(s) => {
                MetricName::parse(s).map_err(|e| anyhow!("invalid dotted metric name: {e}"))
            }
            _ => Err(anyhow!("invalid yaml value type for metric name")),
        }
    }
}

impl<T: IntoIterator<Item = NodeName>> From<T> for MetricName {
    fn from(value: T) -> Self {
        MetricName {
            nodes: value.into_iter().collect(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_iter() {
        let name = MetricName::from([unsafe { NodeName::new_unchecked("foo") }, unsafe {
            NodeName::new_unchecked("bar")
        }]);
        assert_eq!(name.display('.').to_string().as_str(), "foo.bar");
    }

    #[test]
    fn add_prefix() {
        let mut name = MetricName::parse("foo.counter").unwrap();
        let prefix = MetricName::parse_with_delimiter("g3-bar", '-').unwrap();
        name.add_prefix(&prefix);
        assert_eq!(name.display('.').to_string().as_str(), "g3.bar.foo.counter");
    }
}
