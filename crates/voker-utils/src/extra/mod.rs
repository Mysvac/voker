//! Extra collection-like utilities built on top of `alloc`.
//!
//! This module provides small focused data structures that are commonly used
//! by runtime and scheduler internals.
//!
//! - [`ArrayDeque`]: fixed-capacity deque with stack storage.
//! - [`BlockList`]: segmented FIFO queue that reuses drained blocks.
//! - [`BloomFilter`]: compact probabilistic membership filter.
//! - [`PagePool`]: page allocator for bump-style workloads.
//! - [`TypeIdMap`]: type-indexed key-value map.

// -----------------------------------------------------------------------------
// Modules

mod array_deque;
mod block_list;
mod bloom_filter;
mod page_pool;
mod typeid_map;

// -----------------------------------------------------------------------------
// Exports

pub use array_deque::ArrayDeque;
pub use block_list::BlockList;
pub use bloom_filter::BloomFilter;
pub use page_pool::PagePool;
pub use typeid_map::{TypeIdMap, TypeIdMapEntry};
