//! Vector containers with different storage strategies.
//!
//! This module provides three vector types optimized for different scenarios:
//!
//! - [`ArrayVec`]: fixed-capacity vector with inline storage.
//! - [`SmallVec`]: inline-first vector that spills to heap when needed.
//! - [`FastVec`]: inline-first vector that caches an active pointer for fast data-path operations.
//!
//! # Type Selection
//!
//! - Choose [`ArrayVec`] when capacity is known at compile time and must stay fixed.
//! - Choose [`SmallVec`] for general-purpose small-buffer optimization with `Vec`-like behavior.
//! - Choose [`FastVec`] when frequent data operations are performance critical and you can work
//!   through its data handle API.
//!
//! # Examples
//!
//! ```no_run
//! use voker_utils::vec::{ArrayVec, FastVec, SmallVec};
//!
//! let mut a: ArrayVec<i32, 4> = ArrayVec::new();
//! a.extend([1, 2]);
//!
//! let mut s: SmallVec<i32, 4> = SmallVec::new();
//! s.extend([1, 2, 3, 4, 5]);
//!
//! let mut f: FastVec<i32, 4> = FastVec::new();
//! f.data().extend([1, 2, 3]);
//!
//! assert_eq!(a.as_slice(), &[1, 2]);
//! assert_eq!(s.as_slice(), &[1, 2, 3, 4, 5]);
//! assert_eq!(f.as_slice(), &[1, 2, 3]);
//! ```
//!
//! # Notes
//!
//! All three types aim to reduce heap allocations for small workloads.
//! For larger payloads and cross-boundary ownership transfer, converting
//! into [`Vec`](alloc::vec::Vec) is often the most interoperable choice.
#![expect(unsafe_code, reason = "original implementation")]

pub mod array;
pub mod fast;
pub mod small;

mod utils;

pub use array::ArrayVec;
pub use fast::FastVec;
pub use small::SmallVec;
