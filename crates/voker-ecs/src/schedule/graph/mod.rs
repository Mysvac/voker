//! Graph primitives used by the schedule dependency system.
//!
//! Provides directed/undirected graph containers, topological sort,
//! strongly-connected component detection, and a [`Dag`] wrapper that
//! combines a directed graph with a cached topological order.

mod dag;
mod graphs;
mod scc;
mod toposort;

// -----------------------------------------------------------------------------
// Exports

pub use dag::Dag;
pub use graphs::{DiGraph, Direction, Graph, GraphNode, UnGraph};
pub use scc::{SccIterator, SccNodes};
pub use toposort::ToposortError;
