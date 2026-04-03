//! Contents provided to proc macros.
//!
//! Users should not directly use any content here.

// -----------------------------------------------------------------------------
// Macro tools

/// An internal module provided for proc-macro implementation.
pub mod macro_utils {
    // When generating code, using `std` or `alloc` directly is unsafe.
    // Users may be in a `no_std` env or not displaying imported `alloc`.
    //
    // Therefore, proc-macro crate will use this path.
    pub use ::alloc::{
        borrow::{Cow, ToOwned},
        boxed::Box,
        string::ToString,
    };

    // An efficient string concatenation function.
    pub use crate::impls::concat;

    // Shared helper for generated `reflect_clone` implementations.
    pub fn reflect_clone_field<T: crate::Reflect + crate::info::TypePath>(
        source: &T,
    ) -> Result<T, crate::ops::ReflectCloneError> {
        if let Ok(t) = source.reflect_clone()
            && let Ok(val) = t.take::<T>()
        {
            Ok(val)
        } else {
            Err(crate::ops::ReflectCloneError::NotSupport {
                type_path: T::type_path(),
            })
        }
    }

    // auto regisration support
    pub use crate::registry::{AutoRegister, RegisterFn};
    pub use voker_inventory as inv;
}
