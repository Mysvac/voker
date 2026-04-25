use alloc::sync::Arc;

use crate::Reflect;
use crate::info::{CustomAttributes, Generics, Type, TypePath};
use crate::info::{impl_custom_attributes_fn, impl_with_custom_attributes};
use crate::info::{impl_docs_fn, impl_generic_fn, impl_type_fn};

/// Metadata for types whose internals are opaque to the reflection system.
///
/// "Opaque" means the type's internal representation is not exposed — for
/// example primitive types like `u64` or heap-backed types like `String`.
///
/// # Examples
///
/// ```rust
/// use voker_reflect::info::Typed;
///
/// assert!(<String>::type_info().as_opaque().is_ok());
/// ```
#[derive(Debug, Clone)]
pub struct OpaqueInfo {
    ty: Type,
    generics: Generics,
    // Use `Option` to reduce unnecessary heap requests (when empty content).
    custom_attributes: Option<Arc<CustomAttributes>>,
    #[cfg(feature = "reflect_docs")]
    docs: Option<&'static str>,
}

impl OpaqueInfo {
    impl_docs_fn!(docs);
    impl_type_fn!(ty);
    impl_generic_fn!(generics);
    impl_custom_attributes_fn!(custom_attributes);
    impl_with_custom_attributes!(custom_attributes);

    /// Create a new [`OpaqueInfo`].
    #[inline]
    pub const fn new<T: Reflect + TypePath + ?Sized>() -> Self {
        Self {
            ty: Type::of::<T>(),
            generics: Generics::new(),
            custom_attributes: None,
            #[cfg(feature = "reflect_docs")]
            docs: None,
        }
    }
}
