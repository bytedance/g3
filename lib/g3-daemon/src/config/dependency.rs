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

// the graph is constructed with edges (node, node), where node is their index
pub fn sort_nodes_in_dependency_graph(edges: Vec<(usize, usize)>) -> Result<Vec<usize>, usize> {
    use petgraph::{algo::toposort, graph::Graph, Directed};

    if edges.is_empty() {
        return Ok(Vec::new());
    }

    let g = Graph::<usize, (), Directed, usize>::from_edges(edges.as_slice());
    match toposort(&g, None) {
        Ok(nodes) => {
            let mut r = Vec::<usize>::new();
            for node_id in nodes.iter() {
                r.push(node_id.index());
            }
            Ok(r)
        }
        Err(cycle) => Err(g[cycle.node_id()]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_dependency_cycle() {
        let edges = vec![(1, 2), (1, 3)];
        assert!(sort_nodes_in_dependency_graph(edges).is_ok());

        // duplicated edge is allowed
        let edges = vec![(1, 2), (1, 2)];
        assert!(sort_nodes_in_dependency_graph(edges).is_ok());

        let edges = vec![(1, 2), (2, 3), (1, 3)];
        assert!(sort_nodes_in_dependency_graph(edges).is_ok());

        let edges = vec![(1, 2), (2, 3), (3, 1)];
        assert!(sort_nodes_in_dependency_graph(edges).is_err());
    }
}
