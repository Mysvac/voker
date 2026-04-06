#![no_std]

#[expect(unused_imports, reason = "Force dynamic linking main crate.")]
#[expect(clippy::single_component_path_imports, reason = "Force linking.")]
use voker_internal;
