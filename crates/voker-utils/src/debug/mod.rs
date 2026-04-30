//! Debug Tools
//!
//! Only takes effect when the `debug` cargo feature is enabled.
//!
//! The types are available when the `debug` feture is disabled,
//! but internally implementation is empty, no effect (runs normally).

mod name;

pub use name::DebugName;
