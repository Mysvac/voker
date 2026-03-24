use alloc::vec::Vec;
use core::fmt::Debug;
use core::ops::{Deref, DerefMut};

use super::{DiGraph, GraphNode, ToposortError};

// -----------------------------------------------------------------------------
// Dag

/// A directed acyclic graph structure.
#[derive(Clone)]
pub struct Dag<N: GraphNode> {
    /// The underlying directed graph.
    graph: DiGraph<N>,
    /// A cached topological ordering of the graph. This is recomputed when the
    /// graph is modified, and is not valid when `dirty` is true.
    toposort: Vec<N>,
    /// Whether the graph has been modified since the last topological sort.
    dirty: bool,
}

impl<N: GraphNode> Default for Dag<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<N: GraphNode> Dag<N> {
    /// Creates a new directed acyclic graph.
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            toposort: Vec::new(),
            dirty: false,
        }
    }

    pub fn with_capacity(nodes: usize, edges: usize) -> Self {
        Self {
            graph: DiGraph::with_capacity(nodes, edges),
            toposort: Vec::new(),
            dirty: false,
        }
    }

    /// Read-only access to the underlying directed graph.
    #[must_use]
    pub fn graph(&self) -> &DiGraph<N> {
        &self.graph
    }

    /// Mutable access to the underlying directed graph. Marks the graph as dirty.
    #[must_use = "This function marks the graph as dirty, so it should be used."]
    pub fn graph_mut(&mut self) -> &mut DiGraph<N> {
        self.dirty = true;
        &mut self.graph
    }

    /// Returns whether the graph is dirty (i.e., has been modified since the
    /// last topological sort).
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Returns whether the graph is topologically sorted (i.e., not dirty).
    #[must_use]
    pub fn is_toposorted(&self) -> bool {
        !self.dirty
    }

    pub fn ensure_toposorted(&mut self) -> Result<(), ToposortError<N>> {
        if self.dirty {
            // recompute the toposort, reusing the existing allocation
            self.toposort = self.graph.toposort(core::mem::take(&mut self.toposort))?;
            self.dirty = false;
        }
        Ok(())
    }

    /// Returns the cached toposort if the graph is not dirty, otherwise returns
    /// `None`.
    #[must_use = "This method only returns a cached value and does not compute anything."]
    pub fn get_toposort(&self) -> Option<&[N]> {
        if self.dirty {
            None
        } else {
            Some(&self.toposort)
        }
    }

    pub fn toposort(&mut self) -> Result<&[N], ToposortError<N>> {
        self.ensure_toposorted()?;
        Ok(&self.toposort)
    }

    pub fn toposort_and_graph(&mut self) -> Result<(&[N], &DiGraph<N>), ToposortError<N>> {
        self.ensure_toposorted()?;
        Ok((&self.toposort, &self.graph))
    }
}

impl<N: GraphNode> Deref for Dag<N> {
    type Target = DiGraph<N>;

    fn deref(&self) -> &Self::Target {
        self.graph()
    }
}

impl<N: GraphNode> DerefMut for Dag<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.graph_mut()
    }
}

impl<N: GraphNode> Debug for Dag<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.dirty {
            f.debug_struct("Dag")
                .field("graph", &self.graph)
                .field("dirty", &self.dirty)
                .finish()
        } else {
            f.debug_struct("Dag")
                .field("graph", &self.graph)
                .field("toposort", &self.toposort)
                .finish()
        }
    }
}

impl<N: GraphNode> From<DiGraph<N>> for Dag<N> {
    fn from(value: DiGraph<N>) -> Self {
        Self {
            graph: value,
            toposort: Vec::new(),
            dirty: true,
        }
    }
}

impl<N: GraphNode> From<Dag<N>> for DiGraph<N> {
    fn from(value: Dag<N>) -> Self {
        value.graph
    }
}
