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

use std::collections::{BTreeMap, BTreeSet};
use std::rc::{Rc, Weak};

use anyhow::{anyhow, Context};

use g3_types::metrics::NodeName;

pub struct TopoNode {
    name: NodeName,
    children: BTreeMap<NodeName, Weak<TopoNode>>,
    all_children: BTreeMap<NodeName, Weak<TopoNode>>,
}

impl TopoNode {
    fn new(name: NodeName) -> Self {
        TopoNode {
            name,
            children: BTreeMap::new(),
            all_children: BTreeMap::new(),
        }
    }

    pub fn name(&self) -> &NodeName {
        &self.name
    }
}

#[derive(Default)]
pub struct TopoMap {
    map: BTreeMap<NodeName, Rc<TopoNode>>,
}

impl TopoMap {
    fn add_node_internal<F>(
        &mut self,
        name: &NodeName,
        parents: &mut Vec<NodeName>,
        resolve_children: &F,
    ) -> anyhow::Result<Rc<TopoNode>>
    where
        F: Fn(&NodeName) -> Option<BTreeSet<NodeName>>,
    {
        if let Some(v) = self.map.get(name) {
            return Ok(v.clone());
        }

        let mut node = TopoNode::new(name.clone());
        let Some(childrens) = resolve_children(name) else {
            let node = Rc::new(node);
            self.map.insert(name.clone(), node.clone());
            return Ok(node);
        };

        parents.push(name.clone());
        for child_name in childrens {
            if parents.contains(&child_name) {
                return Err(anyhow!("{child_name} has a loop dependency on itself"));
            }

            let child_node = self.add_node_internal(&child_name, parents, resolve_children)?;
            node.children
                .insert(child_name.clone(), Rc::downgrade(&child_node));
            node.all_children
                .insert(child_name.clone(), Rc::downgrade(&child_node));
            for (leaf_name, leaf_node) in &child_node.all_children {
                if leaf_name.eq(name) {
                    return Err(anyhow!(
                        "{name}->{child_name} has a loop dependency on {name}"
                    ));
                }
                node.all_children
                    .insert(leaf_name.clone(), leaf_node.clone());
            }
        }
        parents.pop();

        let node = Rc::new(node);
        self.map.insert(name.clone(), node.clone());
        Ok(node)
    }

    pub fn add_node<F>(&mut self, name: &NodeName, resolve_children: &F) -> anyhow::Result<()>
    where
        F: Fn(&NodeName) -> Option<BTreeSet<NodeName>>,
    {
        if self.map.contains_key(name) {
            return Ok(());
        }

        let mut parents = Vec::new();
        self.add_node_internal(name, &mut parents, resolve_children)
            .context(format!(
                "error detected when checking dependency chain {:?}",
                parents
            ))?;
        Ok(())
    }

    pub fn sorted_nodes(&self) -> Vec<Rc<TopoNode>> {
        let mut all_nodes = self.map.values().cloned().collect::<Vec<_>>();
        all_nodes.sort_by(|a, b| {
            let a_count = a.all_children.len();
            let b_count = b.all_children.len();
            a_count.cmp(&b_count)
        });
        all_nodes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn single() {
        let get_child = |name: &NodeName| match name.as_str() {
            _ => None,
        };

        let mut topo_map = TopoMap::default();
        topo_map
            .add_node(&NodeName::from_str("a").unwrap(), &get_child)
            .unwrap();
        let sorted_nodes = topo_map.sorted_nodes();
        assert_eq!(sorted_nodes.len(), 1);
        assert_eq!(sorted_nodes[0].name().as_str(), "a");
    }

    #[test]
    fn two_unrelated() {
        let get_child = |name: &NodeName| match name.as_str() {
            _ => None,
        };

        let mut topo_map = TopoMap::default();
        topo_map
            .add_node(&NodeName::from_str("a").unwrap(), &get_child)
            .unwrap();
        topo_map
            .add_node(&NodeName::from_str("b").unwrap(), &get_child)
            .unwrap();
        let sorted_nodes = topo_map.sorted_nodes();
        assert_eq!(sorted_nodes.len(), 2);
    }

    #[test]
    fn two_directed() {
        let get_child = |name: &NodeName| match name.as_str() {
            "a" => {
                let mut set = BTreeSet::new();
                set.insert(NodeName::from_str("b").unwrap());
                Some(set)
            }
            _ => None,
        };

        let mut topo_map = TopoMap::default();
        topo_map
            .add_node(&NodeName::from_str("a").unwrap(), &get_child)
            .unwrap();
        topo_map
            .add_node(&NodeName::from_str("b").unwrap(), &get_child)
            .unwrap();
        let sorted_nodes = topo_map.sorted_nodes();
        assert_eq!(sorted_nodes.len(), 2);
        assert_eq!(sorted_nodes[0].name().as_str(), "b");
        assert_eq!(sorted_nodes[1].name().as_str(), "a");
    }

    #[test]
    fn two_cycled() {
        let get_child = |name: &NodeName| match name.as_str() {
            "a" => {
                let mut set = BTreeSet::new();
                set.insert(NodeName::from_str("b").unwrap());
                Some(set)
            }
            "b" => {
                let mut set = BTreeSet::new();
                set.insert(NodeName::from_str("a").unwrap());
                Some(set)
            }
            _ => None,
        };

        let mut topo_map = TopoMap::default();
        assert!(topo_map
            .add_node(&NodeName::from_str("a").unwrap(), &get_child)
            .is_err());
    }

    #[test]
    fn three_directed() {
        let get_child = |name: &NodeName| match name.as_str() {
            "a" => {
                let mut set = BTreeSet::new();
                set.insert(NodeName::from_str("b").unwrap());
                Some(set)
            }
            "b" => {
                let mut set = BTreeSet::new();
                set.insert(NodeName::from_str("c").unwrap());
                Some(set)
            }
            _ => None,
        };

        let mut topo_map = TopoMap::default();
        topo_map
            .add_node(&NodeName::from_str("a").unwrap(), &get_child)
            .unwrap();
        topo_map
            .add_node(&NodeName::from_str("b").unwrap(), &get_child)
            .unwrap();
        topo_map
            .add_node(&NodeName::from_str("c").unwrap(), &get_child)
            .unwrap();
        let sorted_nodes = topo_map.sorted_nodes();
        assert_eq!(sorted_nodes.len(), 3);
        assert_eq!(sorted_nodes[0].name().as_str(), "c");
        assert_eq!(sorted_nodes[1].name().as_str(), "b");
        assert_eq!(sorted_nodes[2].name().as_str(), "a");
    }

    #[test]
    fn three_split() {
        let get_child = |name: &NodeName| match name.as_str() {
            "a" => {
                let mut set = BTreeSet::new();
                set.insert(NodeName::from_str("b").unwrap());
                Some(set)
            }
            _ => None,
        };

        let mut topo_map = TopoMap::default();
        topo_map
            .add_node(&NodeName::from_str("a").unwrap(), &get_child)
            .unwrap();
        topo_map
            .add_node(&NodeName::from_str("b").unwrap(), &get_child)
            .unwrap();
        topo_map
            .add_node(&NodeName::from_str("c").unwrap(), &get_child)
            .unwrap();
        let sorted_nodes = topo_map.sorted_nodes();
        assert_eq!(sorted_nodes.len(), 3);
        assert_eq!(sorted_nodes[2].name().as_str(), "a");
    }

    #[test]
    fn three_cycled() {
        let get_child = |name: &NodeName| match name.as_str() {
            "a" => {
                let mut set = BTreeSet::new();
                set.insert(NodeName::from_str("b").unwrap());
                Some(set)
            }
            "b" => {
                let mut set = BTreeSet::new();
                set.insert(NodeName::from_str("c").unwrap());
                Some(set)
            }
            "c" => {
                let mut set = BTreeSet::new();
                set.insert(NodeName::from_str("a").unwrap());
                Some(set)
            }
            _ => None,
        };

        let mut topo_map = TopoMap::default();
        assert!(topo_map
            .add_node(&NodeName::from_str("a").unwrap(), &get_child)
            .is_err());
    }

    #[test]
    fn many_cycled() {
        let get_child = |name: &NodeName| match name.as_str() {
            "a" => {
                let mut set = BTreeSet::new();
                set.insert(NodeName::from_str("b").unwrap());
                set.insert(NodeName::from_str("c").unwrap());
                Some(set)
            }
            "b" => {
                let mut set = BTreeSet::new();
                set.insert(NodeName::from_str("e").unwrap());
                Some(set)
            }
            "c" => {
                let mut set = BTreeSet::new();
                set.insert(NodeName::from_str("d").unwrap());
                Some(set)
            }
            "d" => {
                let mut set = BTreeSet::new();
                set.insert(NodeName::from_str("a").unwrap());
                Some(set)
            }
            _ => None,
        };

        let mut topo_map = TopoMap::default();
        topo_map
            .add_node(&NodeName::from_str("b").unwrap(), &get_child)
            .unwrap();
        assert!(topo_map
            .add_node(&NodeName::from_str("a").unwrap(), &get_child)
            .is_err());
    }
}
