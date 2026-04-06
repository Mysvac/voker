#![doc = include_str!("../README.md")]
#![no_std]

// -----------------------------------------------------------------------------
// No STD Support

extern crate alloc;

// -----------------------------------------------------------------------------
// Modules

mod cold_path;
mod range_invoke;

pub mod extra;
pub mod hash;
pub mod index;
pub mod num;
pub mod vec;

// -----------------------------------------------------------------------------
// Top-level exports

pub use cold_path::cold_path;
