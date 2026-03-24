#![doc = include_str!("../README.md")]
#![expect(unsafe_code, reason = "Raw pointers are inherently unsafe.")]
#![no_std]

// -----------------------------------------------------------------------------
// Modules

mod thin_slice;
mod type_erased;

// -----------------------------------------------------------------------------
// Top-level exports

pub use thin_slice::{ThinSlice, ThinSliceMut};
pub use type_erased::{OwningPtr, Ptr, PtrMut};
