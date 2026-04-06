#![no_std]

pub use voker_internal::*;

#[cfg(all(feature = "dynlib", not(target_family = "wasm")))]
#[expect(unused_imports, reason = "Force dynamic linking main crate.")]
#[expect(clippy::single_component_path_imports, reason = "Force linking.")]
use voker_internal;
