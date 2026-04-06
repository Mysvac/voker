//! Contents provided to proc macros.
//!
//! Users should not directly use any content here.

// -----------------------------------------------------------------------------
// Macro tools

/// An internal module provided for proc-macro implementation.
pub mod macro_utils {
    pub use ::alloc::boxed::Box;
}
