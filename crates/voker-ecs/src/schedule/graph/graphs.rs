use alloc::vec::Vec;
use core::fmt::Debug;
use core::hash::Hash;

use voker_utils::hash::HashSet;
use voker_utils::index::IndexMap;

use Direction::{Incoming, Outgoing};

// -----------------------------------------------------------------------------
// Graph

/// Edge direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Direction {
    /// An `Outgoing` edge is an outward edge *from* the current node.
    Outgoing = 0,
    /// An `Incoming` edge is an inbound edge *to* the current node.
    Incoming = 1,
}

impl Direction {
    /// Return the opposite `Direction`.
    #[inline]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Outgoing => Self::Incoming,
            Self::Incoming => Self::Outgoing,
        }
    }
}

/// Types that can be used as node identifiers in a [`DiGraph`]/[`UnGraph`].
pub trait GraphNode: Copy + Hash + Eq + Ord + Debug {
    /// The type that packs and unpacks this [`GraphNode`] with a [`Direction`].
    /// This is used to save space in the graph's adjacency list.
    type Link: Copy + Debug + From<(Self, Direction)> + Into<(Self, Direction)>;
    /// The type that packs and unpacks this [`GraphNode`] with another
    /// [`GraphNode`]. This is used to save space in the graph's edge list.
    type Edge: Copy + Hash + Eq + Debug + From<(Self, Self)> + Into<(Self, Self)>;
}

/// `Graph<DIRECTED>` is a graph datastructure using an associative array
/// of its node weights of some [`GraphNode`].
///
/// It uses a combined adjacency list and sparse adjacency matrix
/// representation, using **O(|N| + |E|)** space, and allows testing for edge
/// existence in constant time.
///
/// `Graph` is parameterized over:
///
/// - Constant generic bool `DIRECTED` determines whether the graph edges are directed or
///   undirected.
/// - The `GraphNode` type `N`, which is used as the node weight.
/// - The `BuildHasher` `S`.
///
/// You can use the type aliases `UnGraph` and `DiGraph` for convenience.
///
/// `Graph` does not allow parallel edges, but self loops are allowed.
#[derive(Clone)]
pub struct Graph<const DIRECTED: bool, N: GraphNode> {
    nodes: IndexMap<N, Vec<N::Link>>,
    edges: HashSet<N::Edge>,
}

pub type DiGraph<N> = Graph<true, N>;
pub type UnGraph<N> = Graph<false, N>;

impl<const DIRECTED: bool, N: GraphNode> Debug for Graph<DIRECTED, N> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        self.nodes.fmt(f)
    }
}

impl<const DIRECTED: bool, N: GraphNode> Default for Graph<DIRECTED, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const DIRECTED: bool, N: GraphNode> Graph<DIRECTED, N> {
    pub const fn new() -> Self {
        Self {
            nodes: IndexMap::new(),
            edges: HashSet::new(),
        }
    }

    pub fn with_capacity(nodes: usize, edges: usize) -> Self {
        Self {
            nodes: IndexMap::with_capacity(nodes),
            edges: HashSet::with_capacity(edges),
        }
    }

    #[inline]
    fn edge_key(a: N, b: N) -> N::Edge {
        let (a, b) = if DIRECTED || a <= b { (a, b) } else { (b, a) };

        N::Edge::from((a, b))
    }

    fn remove_link(&mut self, x: N, y: N, dir: Direction) -> bool {
        if let Some(links) = self.nodes.get_mut(&x) {
            let index = links
                .iter()
                .copied()
                .map(N::Link::into)
                .position(|link| link == (y, dir));
            if let Some(index) = index {
                links.swap_remove(index);
                return true;
            }
        };
        false
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn reserve_nodes(&mut self, additional: usize) {
        self.nodes.reserve(additional);
    }

    pub fn reserve_edges(&mut self, additional: usize) {
        self.edges.reserve(additional);
    }

    pub fn contains_node(&self, n: N) -> bool {
        self.nodes.contains_key(&n)
    }

    pub fn contains_edge(&self, a: N, b: N) -> bool {
        self.edges.contains(&Self::edge_key(a, b))
    }

    pub fn insert_node(&mut self, n: N) {
        self.nodes.entry(n).or_default();
    }

    pub fn remove_node(&mut self, n: N) {
        let Some(links) = self.nodes.swap_remove(&n) else {
            return;
        };

        links.into_iter().map(N::Link::into).for_each(|(to, dir)| {
            let (edge, rdir) = if dir == Outgoing {
                (Self::edge_key(n, to), Incoming)
            } else {
                (Self::edge_key(to, n), Outgoing)
            };

            self.remove_link(to, n, rdir);
            self.edges.remove(&edge);
        })
    }

    pub fn insert_edge(&mut self, a: N, b: N) {
        if self.edges.insert(Self::edge_key(a, b)) {
            // insert in the adjacency list if it's a new edge
            self.nodes
                .entry(a)
                .or_insert_with(|| Vec::with_capacity(1))
                .push(N::Link::from((b, Outgoing)));
            if a != b {
                // self loops don't have the Incoming entry
                self.nodes
                    .entry(b)
                    .or_insert_with(|| Vec::with_capacity(1))
                    .push(N::Link::from((a, Incoming)));
            }
        }
    }

    pub fn remove_edge(&mut self, a: N, b: N) -> bool {
        let exist1 = self.remove_link(a, b, Outgoing);
        let exist2 = if a != b {
            self.remove_link(b, a, Incoming)
        } else {
            core::hint::cold_path();
            exist1
        };
        let weight = self.edges.remove(&Self::edge_key(a, b));
        debug_assert!(exist1 == exist2 && exist1 == weight);
        weight
    }

    pub fn nodes(&self) -> impl DoubleEndedIterator<Item = N> + ExactSizeIterator<Item = N> + '_ {
        self.nodes.keys().copied()
    }

    pub fn all_edges(&self) -> impl ExactSizeIterator<Item = (N, N)> + '_ {
        self.edges.iter().copied().map(N::Edge::into)
    }

    pub fn edges(&self, a: N) -> impl DoubleEndedIterator<Item = (N, N)> + '_ {
        let iter = match self.nodes.get(&a) {
            Some(neigh) => neigh.iter(),
            None => [].iter(),
        };

        iter.copied()
            .map(N::Link::into)
            .filter_map(move |(b, dir)| (!DIRECTED || dir == Outgoing).then_some((a, b)))
    }

    pub fn neighbors(&self, a: N) -> impl DoubleEndedIterator<Item = N> + '_ {
        let iter = match self.nodes.get(&a) {
            Some(neigh) => neigh.iter(),
            None => [].iter(),
        };

        iter.copied()
            .map(N::Link::into)
            .filter_map(|(n, dir)| (!DIRECTED || dir == Outgoing).then_some(n))
    }

    pub fn edges_directed(
        &self,
        a: N,
        dir: Direction,
    ) -> impl DoubleEndedIterator<Item = (N, N)> + '_ {
        let iter = match self.nodes.get(&a) {
            Some(neigh) => neigh.iter(),
            None => [].iter(),
        };

        iter.copied()
            .map(N::Link::into)
            .filter_map(move |(b, d)| (!DIRECTED || d == dir || b == a).then_some((a, b)))
    }

    pub fn neighbors_directed(
        &self,
        a: N,
        dir: Direction,
    ) -> impl DoubleEndedIterator<Item = N> + '_ {
        let iter = match self.nodes.get(&a) {
            Some(neigh) => neigh.iter(),
            None => [].iter(),
        };

        iter.copied()
            .map(N::Link::into)
            .filter_map(move |(n, d)| (!DIRECTED || d == dir || n == a).then_some(n))
    }

    pub(super) fn to_index(&self, ix: N) -> usize {
        self.nodes.get_index_of(&ix).unwrap()
    }
}
