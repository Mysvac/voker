//! Access declaration and conflict analysis model for ECS systems.
//!
//! # Design pattern overview
//!
//! The access model is intentionally split into three layers:
//! 1. [`AccessParam`]: fine-grained component access for one logical query path.
//! 2. [`FilterParam`]: a canonical key describing query `with` / `without` constraints.
//! 3. [`AccessTable`]: full per-system access summary consumed by scheduler conflict checks.
//!
//! This layering keeps conflict analysis both strict and practical:
//! - component-level conflicts are tracked with explicit read/write sets,
//! - query filter disjointness reduces false-positive conflicts,
//! - world/resource/query accesses are merged into one schedulable table.
//!
//! # Why both `AccessParam` and `AccessTable` exist
//!
//! `AccessParam` answers: "is this one parameter configuration internally valid?"
//! `AccessTable` answers: "can two systems run in parallel?"
//!
//! A custom [`SystemParam`](crate::system::SystemParam) implementation typically:
//! 1. builds one or more `AccessParam` values,
//! 2. maps each query branch to one or more `FilterParam` keys,
//! 3. inserts results into `AccessTable` via `set_query` / resource setters.
//!
//! # Integration path
//!
//! Access registration is declared during
//! [`System::initialize`](crate::system::System::initialize), then stored by
//! the schedule and reused for runtime conflict decisions.

mod data;
mod filter;
mod table;

pub use data::AccessParam;
pub use filter::{FilterParam, FilterParamBuilder};
pub use table::AccessTable;
