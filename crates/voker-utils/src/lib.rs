#![doc = include_str!("../README.md")]
#![no_std]

// -----------------------------------------------------------------------------
// No STD Support

extern crate alloc;

// -----------------------------------------------------------------------------
// Modules

mod range_invoke;

pub mod debug;
pub mod extra;
pub mod hash;
pub mod index;
pub mod num;
pub mod smol;
pub mod vec;

// -----------------------------------------------------------------------------
// Exports
