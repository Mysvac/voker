#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
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

// pub const fn is_send<T: Send>() {}
// pub const fn is_sync<T: Sync>() {}
// pub const fn is_unwind_safe<T: core::panic::UnwindSafe>() {}
// pub const fn is_ref_unwind_safe<T: core::panic::RefUnwindSafe>() {}
