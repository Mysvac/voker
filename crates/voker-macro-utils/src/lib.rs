#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![expect(clippy::std_instead_of_alloc, reason = "proc-macro crate")]

extern crate proc_macro;

// -----------------------------------------------------------------------------
// Modules

mod manifest;

// -----------------------------------------------------------------------------
// Exports

pub mod full_path;

#[doc(inline)]
pub use manifest::crate_path;

/// Resolve a crate name into a canonical `syn::Path`.
///
/// This macro is a thin wrapper around [`crate_path()`]. It stringifies the
/// provided path token and asks `manifest` resolution logic to map it to an
/// absolute path.
///
/// Can **not** be used for third-party crates that with `voker_` prefix.
///
/// # Examples
///
/// ```no_run
/// let voker_ecs = voker_path::crate_path!(voker_ecs);
/// ```
#[macro_export]
macro_rules! crate_path {
    ($path:path) => {
        $crate::crate_path(::core::stringify!($path))
    };
}
