//! Global storage for world resources.
//!
//! This layer stores resource values independently from entity/component tables
//! and provides typed/untyped resource access helpers.

mod data;
mod set;

pub use data::ResData;
pub use set::ResSet;
