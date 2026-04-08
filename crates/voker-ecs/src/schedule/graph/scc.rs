use core::iter::FusedIterator;

use voker_utils::vec::SmallVec;

use super::{DiGraph, GraphNode};

// -----------------------------------------------------------------------------
// SccIterator

/// A node set in a *strongly connected components*.
///
/// A valid scheduling graph in ECS should not contain cycles,
/// meaning all SCCs are single nodes. Therefore we use `SmallVec`
/// for optimization, avoiding heap allocation in most scene.
///
/// If `N` is `8 bytes`, then `SmallVec<N, 2>` is exactly the same
/// size as `Vec<N>`.
pub type SccNodes<N> = SmallVec<N, 2>;

/// A specialized iterator that returns slices of SCCs, which is faster
/// than the regular iterator because it avoids copying values.
///
/// The reason this trait exists is that `Iterator` itself cannot return
/// references tied to the iterator's own lifetime.
///
/// Obtained via [`DiGraph::iter_sccs`].
pub trait SccIterator<N: GraphNode>: FusedIterator<Item = SccNodes<N>> {
    /// Returns the next strongly connected component as a slice.
    fn next_scc(&mut self) -> Option<&[N]>;
}

impl<N: GraphNode> DiGraph<N> {
    /// Create an iterator over *strongly connected components*.
    ///
    /// Using Algorithm in [A Space-Efficient Algorithm for Finding Strongly Connected Components][1]
    /// by David J. Pierce, which is a memory-efficient variation of [Tarjan's algorithm][2].
    ///
    /// [1]: https://www.sciencedirect.com/science/article/abs/pii/S0020019015001532
    /// [2]: https://en.wikipedia.org/wiki/Tarjan%27s_strongly_connected_components_algorithm
    ///
    /// # Output
    /// - Each yielded item is a strongly connected component.
    /// - The order of nodes within each SCC is arbitrary but deterministic for a given graph.
    /// - The order of SCCs themselves is their postorder (reverse topological sort).
    ///
    /// # Complexity
    /// Let `N` be the number of nodes and `M` the number of edges:
    /// - Time: O(N + M)
    /// - Space: O(N)
    pub fn iter_sccs(&self) -> impl SccIterator<N> + '_ {
        tarjan_scc::new_tarjan_scc(self)
    }
}

// -----------------------------------------------------------------------------
// Tarjan SCC

mod tarjan_scc {
    use alloc::vec::Vec;
    use core::{iter::FusedIterator, num::NonZeroUsize};

    use super::{SccIterator, SccNodes};
    use crate::schedule::{DiGraph, GraphNode};

    pub(super) fn new_tarjan_scc<N: GraphNode>(graph: &DiGraph<N>) -> impl SccIterator<N> + '_ {
        let unchecked_nodes = graph.nodes();

        let nodes = graph
            .nodes()
            .map(|node| NodeData {
                root: None,
                pending: None,
                neighbors: graph.neighbors(node),
            })
            .collect::<Vec<_>>();

        TarjanScc {
            graph,
            unchecked_nodes,
            nodes,
            dfs_index: 1,          // Invariant: dfs_index < scc_index at all times.
            scc_index: usize::MAX, // Will hold if scc_index is initialized to number of nodes - 1 or higher.
            stack: Vec::new(),
            visitation_stack: Vec::new(),
            scc_start: None,
            scc_len: None,
        }
    }

    struct NodeData<N: GraphNode, Neighbors: Iterator<Item = N>> {
        /// - None: unvisited
        /// - Some: `dfs_index` (searching) or `scc_index` (complete)
        root: Option<NonZeroUsize>,
        pending: Option<N>,
        neighbors: Neighbors,
    }

    /// A state for computing the *strongly connected components* using [Tarjan's algorithm][1].
    ///
    /// This is based on [`TarjanScc`] from [`petgraph`].
    ///
    /// [1]: https://en.wikipedia.org/wiki/Tarjan%27s_strongly_connected_components_algorithm
    /// [`petgraph`]: https://docs.rs/petgraph/0.6.5/petgraph/
    /// [`TarjanScc`]: https://docs.rs/petgraph/0.6.5/petgraph/algo/struct.TarjanScc.html
    struct TarjanScc<'graph, N, AllNodes, Neighbors>
    where
        N: GraphNode,
        AllNodes: Iterator<Item = N>,
        Neighbors: Iterator<Item = N>,
    {
        /// Source of truth [`DiGraph`]
        graph: &'graph DiGraph<N>,
        /// An [`Iterator`] of [`GraphNode`]s from the `graph` which may not have been visited yet.
        unchecked_nodes: AllNodes,
        /// Information about each [`GraphNode`], including a possible SCC index and an
        /// [`Iterator`] of possibly unvisited neighbors.
        nodes: Vec<NodeData<N, Neighbors>>,
        /// The index of the next SCC
        dfs_index: usize,
        /// A count of potentially remaining SCCs
        scc_index: usize,
        /// A stack of [`GraphNode`]s where a SCC will be found starting at the top of the stack.
        stack: Vec<N>,
        /// A stack of [`GraphNode`]s which need to be visited to determine which SCC they belong to.
        visitation_stack: Vec<(N, bool)>,
        /// An index into the `stack` indicating the starting point of a SCC.
        scc_start: Option<usize>,
        /// An adjustment to the `index` which will be applied once the current SCC is found.
        scc_len: Option<usize>,
    }

    impl<'graph, N, A, Neighbors> TarjanScc<'graph, N, A, Neighbors>
    where
        N: GraphNode + Copy,
        A: Iterator<Item = N>,
        Neighbors: Iterator<Item = N>,
    {
        /// Compute the next *strongly connected component* using Algorithm 3 in
        /// [A Space-Efficient Algorithm for Finding Strongly Connected Components][1] by David J. Pierce,
        /// which is a memory-efficient variation of [Tarjan's algorithm][2].
        ///
        /// [1]: https://homepages.ecs.vuw.ac.nz/~djp/files/P05.pdf
        /// [2]: https://en.wikipedia.org/wiki/Tarjan%27s_strongly_connected_components_algorithm
        ///
        /// Returns `Some` for each strongly connected component (scc).
        /// The order of node ids within each scc is arbitrary, but the order of
        /// the sccs is their postorder (reverse topological sort).
        fn next_scc(&mut self) -> Option<&[N]> {
            // Cleanup from possible previous iteration
            if let (Some(start), Some(len)) = (self.scc_start.take(), self.scc_len.take()) {
                // self.stack.truncate(start);
                unsafe {
                    self.stack.set_len(start);
                }
                self.dfs_index -= len; // Backtrack index back to where it was before we ever encountered the component.
                self.scc_index -= 1;
            }

            'out: loop {
                // If there are items on the visitation stack, then we haven't finished visiting
                // the node at the bottom of the stack yet.
                // Must visit all nodes in the stack from top to bottom before visiting the next node.
                while let Some((v, v_is_local_root)) = self.visitation_stack.pop() {
                    // If this visitation finds a complete SCC, return it immediately.
                    if let Some(start) = self.visit_once(v, v_is_local_root) {
                        return Some(&self.stack[start..]);
                    };
                }

                loop {
                    // Get the next node to check, otherwise we're done and can return None.
                    let node = self.unchecked_nodes.next()?;
                    let node_index = self.graph.to_index(node);
                    let unvisited = self.nodes[node_index].root.is_none();
                    // If this node hasn't already been visited (e.g., it was the neighbor of a
                    // previously checked node) add it to the visitation stack.
                    if unvisited {
                        self.visitation_stack.push((node, true));
                        continue 'out;
                    }
                }
            }
        }

        /// Attempt to find the starting point on the stack for a new SCC without visiting neighbors.
        /// If a visitation is required, this will return `None` and mark the required neighbor and the
        /// current node as in need of visitation again.
        /// If no SCC can be found in the current visitation stack, returns `None`.
        fn visit_once(&mut self, v: N, mut v_is_local_root: bool) -> Option<usize> {
            let node_index_v = self.graph.to_index(v);

            let node_v = &mut self.nodes[node_index_v];

            if node_v.root.is_none() {
                node_v.root = NonZeroUsize::new(self.dfs_index);
                self.dfs_index += 1;
            }

            if let Some(w) = node_v.pending.take() {
                let node_index_w = self.graph.to_index(w);
                if self.nodes[node_index_v].root > self.nodes[node_index_w].root {
                    self.nodes[node_index_v].root = self.nodes[node_index_w].root;
                    v_is_local_root = false;
                }
            }

            while let Some(w) = self.nodes[node_index_v].neighbors.next() {
                let node_index_w = self.graph.to_index(w);

                // If a neighbor hasn't been visited yet...
                if self.nodes[node_index_w].root.is_none() {
                    // Push the current node and the neighbor back onto the visitation stack.
                    // On the next execution of `visit_once`, the neighbor will be visited.
                    self.visitation_stack.push((v, v_is_local_root));
                    self.visitation_stack.push((w, true));
                    self.nodes[node_index_v].pending = Some(w);

                    return None;
                }

                if self.nodes[node_index_v].root > self.nodes[node_index_w].root {
                    self.nodes[node_index_v].root = self.nodes[node_index_w].root;
                    v_is_local_root = false;
                }
            }

            if !v_is_local_root {
                // Stack is filled up when backtracking, unlike in Tarjans original algorithm.
                self.stack.push(v);
                return None;
            }

            // Pop the stack and generate an SCC.
            let nodes = &mut self.nodes;
            let scc_id = NonZeroUsize::new(self.scc_index);
            let mut scc_len = 1;
            let scc_start = self
                .stack
                .iter()
                .rposition(|&w| {
                    let node_index_w = self.graph.to_index(w);

                    if nodes[node_index_w].root < nodes[node_index_v].root {
                        true
                    } else {
                        nodes[node_index_w].root = scc_id;
                        scc_len += 1;
                        false
                    }
                })
                .map(|x| x + 1)
                .unwrap_or_default();

            nodes[node_index_v].root = scc_id;
            self.stack.push(v); // Pushing the component root to the back right before getting rid of it is somewhat ugly, but it lets it be included in f.

            self.scc_start = Some(scc_start);
            self.scc_len = Some(scc_len);

            Some(scc_start)
        }
    }

    impl<'graph, N, A, Neighbors> Iterator for TarjanScc<'graph, N, A, Neighbors>
    where
        N: GraphNode,
        A: Iterator<Item = N>,
        Neighbors: Iterator<Item = N>,
    {
        type Item = SccNodes<N>;

        fn next(&mut self) -> Option<SccNodes<N>> {
            let ret = self.next_scc()?;
            Some(SccNodes::from_slice(ret))
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            // There can be no more than the number of nodes in a graph worth of SCCs
            (0, Some(self.nodes.len()))
        }
    }

    impl<N, A, Neighbors> FusedIterator for TarjanScc<'_, N, A, Neighbors>
    where
        N: GraphNode,
        A: Iterator<Item = N>,
        Neighbors: Iterator<Item = N>,
    {
    }

    impl<N, A, Neighbors> SccIterator<N> for TarjanScc<'_, N, A, Neighbors>
    where
        N: GraphNode,
        A: Iterator<Item = N>,
        Neighbors: Iterator<Item = N>,
    {
        fn next_scc(&mut self) -> Option<&[N]> {
            TarjanScc::next_scc(self)
        }
    }
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use crate::schedule::{DiGraph, Direction, GraphNode, SccIterator};
    use alloc::vec::Vec;

    #[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
    pub struct Node(i32);

    impl GraphNode for Node {
        type Link = (Node, Direction);
        type Edge = (Node, Node);
    }

    #[test]
    fn a_b_c_a() {
        let mut graph = DiGraph::with_capacity(3, 3);

        graph.insert_edge(Node(1), Node(2));
        graph.insert_edge(Node(2), Node(3));
        graph.insert_edge(Node(3), Node(1));

        let mut tarjan = graph.iter_sccs();
        assert_eq!(tarjan.next_scc().unwrap(), &[Node(3), Node(2), Node(1)]);
        assert_eq!(tarjan.next_scc(), None);
    }

    #[test]
    fn multi_region() {
        use alloc::vec;
        let mut graph = DiGraph::default();

        graph.insert_edge(Node(1), Node(2));
        graph.insert_edge(Node(2), Node(1));

        graph.insert_edge(Node(2), Node(3));
        graph.insert_edge(Node(3), Node(2));

        graph.insert_edge(Node(4), Node(5));
        graph.insert_edge(Node(5), Node(4));

        graph.insert_edge(Node(6), Node(2));

        let sccs = graph.iter_sccs().map(|scc| scc.to_vec()).collect::<Vec<_>>();

        assert_eq!(
            sccs,
            vec![
                vec![Node(3), Node(2), Node(1)],
                vec![Node(5), Node(4)],
                vec![Node(6)]
            ]
        );
    }
}
