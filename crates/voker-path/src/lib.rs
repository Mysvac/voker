#![doc = include_str!("../README.md")]
#![expect(clippy::std_instead_of_alloc, reason = "proc-macro crate")]

extern crate proc_macro;

// -----------------------------------------------------------------------------
// Modules

mod manifest;

// -----------------------------------------------------------------------------
// Exports

pub mod full_path;
pub use manifest::crate_path;

/// Resolve a crate name into a canonical `syn::Path`.
///
/// This macro is a thin wrapper around [`crate_path()`]. It stringifies the
/// provided path token and asks `manifest` resolution logic to map it to an
/// absolute path.
///
/// # Examples
///
/// ```no_run
/// let path = voker_path::crate_path!(syn);
/// ```
#[macro_export]
macro_rules! crate_path {
    ($path:path) => {
        $crate::crate_path(::core::stringify!($path))
    };
}
