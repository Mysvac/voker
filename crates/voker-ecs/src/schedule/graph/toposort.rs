use crate::schedule::SccIterator;
use alloc::vec::Vec;
use thiserror::Error;
use voker_utils::hash::{HashMap, HashSet};

use super::{DiGraph, GraphNode};

// -----------------------------------------------------------------------------
// toposort

#[derive(Error, Debug)]
pub enum ToposortError<N: GraphNode> {
    /// A self-loop was detected.
    #[error("self-loop detected at node `{0:?}`")]
    Loop(N),
    /// Cycles were detected.
    #[error("cycles detected: {0:?}")]
    Cycle(Vec<Vec<N>>),
}

impl<N: GraphNode> DiGraph<N> {
    pub fn toposort(&self, mut scratch: Vec<N>) -> Result<Vec<N>, ToposortError<N>> {
        // Check explicitly for self-edges.
        // `iter_sccs` won't report them as cycles because they still form components of one node.
        if let Some((node, _)) = self.all_edges().find(|(x, y)| x == y) {
            core::hint::cold_path();
            return Err(ToposortError::Loop(node));
        }

        // Tarjan's SCC algorithm returns elements in *reverse* topological order.
        scratch.clear();
        scratch.reserve_exact(self.node_count().saturating_sub(scratch.capacity()));
        let mut top_sorted_nodes: Vec<N> = scratch;
        let mut sccs_with_cycles: Vec<Vec<N>> = Vec::new();

        // A strongly-connected component is a group of nodes
        // who can all reach each other through one or more paths.
        let mut scc_iter = self.iter_sccs();
        // `SccIterator::next_scc` is faster then `Iterator::next`,
        // Because we do not need copy data to `SmallVec`.
        while let Some(scc) = scc_iter.next_scc() {
            top_sorted_nodes.extend_from_slice(scc);
            // If an SCC contains more than one node,
            // there must be at least one cycle within them.
            if scc.len() > 1 {
                core::hint::cold_path();
                sccs_with_cycles.push(Vec::from(scc));
            }
        }

        if sccs_with_cycles.is_empty() {
            // reverse to get topological order
            top_sorted_nodes.reverse();
            Ok(top_sorted_nodes)
        } else {
            core::hint::cold_path();
            let mut cycles = Vec::new();
            for scc in &sccs_with_cycles {
                cycles.append(&mut self.simple_cycles_in_component(scc));
            }

            Err(ToposortError::Cycle(cycles))
        }
    }

    /// Returns the simple cycles in a strongly-connected component of a directed graph.
    ///
    /// The algorithm implemented comes from
    /// ["Finding all the elementary circuits of a directed graph"][1] by D. B. Johnson.
    ///
    /// [1]: https://doi.org/10.1137/0204007
    fn simple_cycles_in_component(&self, scc: &[N]) -> Vec<Vec<N>> {
        let mut cycles: Vec<Vec<N>> = Vec::new();

        let mut sccs: Vec<Vec<N>> = Vec::new();
        sccs.push(scc.to_vec());

        while let Some(mut scc) = sccs.pop() {
            // only look at nodes and edges in this strongly-connected component
            let mut subgraph = DiGraph::<N>::with_capacity(scc.len(), 0);
            for &node in &scc {
                subgraph.insert_node(node);
            }

            for &node in &scc {
                for successor in self.neighbors(node) {
                    if subgraph.contains_node(successor) {
                        subgraph.insert_edge(node, successor);
                    }
                }
            }

            // path of nodes that may form a cycle
            let mut path: Vec<N> = Vec::with_capacity(subgraph.node_count());
            // we mark nodes as "blocked" to avoid finding permutations of the same cycles
            let mut blocked: HashSet<N> = HashSet::with_capacity(subgraph.node_count());
            // connects nodes along path segments that can't be part of a cycle (given current root)
            // those nodes can be unblocked at the same time
            let mut unblock_together: HashMap<N, HashSet<N>> =
                HashMap::with_capacity(subgraph.node_count());
            // stack for unblocking nodes
            let mut unblock_stack: Vec<N> = Vec::with_capacity(subgraph.node_count());
            // nodes can be involved in multiple cycles
            let mut maybe_in_more_cycles: HashSet<N> =
                HashSet::with_capacity(subgraph.node_count());
            // stack for DFS
            let mut stack = Vec::with_capacity(subgraph.node_count());

            // we're going to look for all cycles that begin and end at this node
            let root = scc.pop().unwrap();
            // start a path at the root
            path.clear();
            path.push(root);
            // mark this node as blocked
            blocked.insert(root);

            // DFS
            stack.clear();
            stack.push((root, subgraph.neighbors(root)));
            while !stack.is_empty() {
                let &mut (ref node, ref mut successors) = stack.last_mut().unwrap();
                if let Some(next) = successors.next() {
                    if next == root {
                        // found a cycle
                        maybe_in_more_cycles.extend(path.iter());
                        cycles.push(path.clone());
                    } else if !blocked.contains(&next) {
                        // first time seeing `next` on this path
                        maybe_in_more_cycles.remove(&next);
                        path.push(next);
                        blocked.insert(next);
                        stack.push((next, subgraph.neighbors(next)));
                        continue;
                    } else {
                        // not first time seeing `next` on this path
                    }
                }

                if successors.peekable().peek().is_none() {
                    if maybe_in_more_cycles.contains(node) {
                        unblock_stack.push(*node);
                        // unblock this node's ancestors
                        while let Some(n) = unblock_stack.pop() {
                            if blocked.remove(&n) {
                                let unblock_predecessors = unblock_together.entry(n).or_default();
                                unblock_stack.extend(unblock_predecessors.iter());
                                unblock_predecessors.clear();
                            }
                        }
                    } else {
                        // if its descendants can be unblocked later, this node will be too
                        for successor in subgraph.neighbors(*node) {
                            unblock_together.entry(successor).or_default().insert(*node);
                        }
                    }

                    // remove node from path and DFS stack
                    path.pop();
                    stack.pop();
                }
            }

            drop(stack);

            // remove node from subgraph
            subgraph.remove_node(root);

            // divide remainder into smaller SCCs
            let mut scc_iter = subgraph.iter_sccs();
            while let Some(scc) = scc_iter.next_scc() {
                if scc.len() > 1 {
                    sccs.push(scc.to_vec());
                }
            }
        }

        cycles
    }
}

#[cfg(test)]
mod tests {
    use crate::schedule::{DiGraph, Direction, GraphNode, ToposortError};
    use alloc::vec;
    use alloc::vec::Vec;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    struct Node(usize);

    impl GraphNode for Node {
        type Link = (Node, Direction);
        type Edge = (Node, Node);
    }

    fn build_graph(edges: &[(usize, usize)]) -> DiGraph<Node> {
        let mut graph = DiGraph::new();
        for &(a, b) in edges {
            graph.insert_edge(Node(a), Node(b));
        }
        graph
    }

    #[test]
    fn simple_dag() {
        // 1 → 2 → 3
        //     ↓ → 4
        let graph = build_graph(&[(1, 2), (2, 3), (2, 4)]);
        let result = graph.toposort(Vec::new()).unwrap();
        // should be the reverse of the post-order traversal result
        assert_eq!(result, [Node(1), Node(2), Node(4), Node(3)]);
    }

    #[test]
    fn detects_self_loop() {
        let mut graph = build_graph(&[(1, 2)]);
        graph.insert_edge(Node(1), Node(1));

        let err = graph.toposort(Vec::new()).unwrap_err();
        match err {
            ToposortError::Loop(node) => assert_eq!(node, Node(1)),
            _ => unreachable!("Expected Loop error"),
        }
    }

    #[test]
    fn detects_simple_cycle() {
        let graph = build_graph(&[(1, 2), (2, 1)]);
        let err = graph.toposort(Vec::new()).unwrap_err();

        match err {
            ToposortError::Cycle(cycles) => {
                assert_eq!(cycles.len(), 1);
                assert_eq!(cycles[0].len(), 2);
            }
            _ => unreachable!("Expected Cycle error"),
        }
    }

    #[test]
    fn empty_graph() {
        let graph = DiGraph::<Node>::new();
        let result = graph.toposort(Vec::new()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn single_node() {
        let mut graph = DiGraph::new();
        graph.insert_node(Node(1));

        let result = graph.toposort(Vec::new()).unwrap();
        assert_eq!(result, vec![Node(1)]);
    }
}
