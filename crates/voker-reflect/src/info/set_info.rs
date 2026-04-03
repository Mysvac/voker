use core::any::{Any, TypeId};

use crate::Reflect;
use crate::info::{Generics, Type, TypeInfo, TypePath, Typed};
use crate::info::{impl_docs_fn, impl_generic_fn, impl_type_fn};
use crate::ops::Set;

/// A container for compile-time set-like info.
///
/// At present, `SetInfo` does not have `CustomAttributes`, which can save memory.
///
/// # Examples
///
/// ```rust
/// use voker_reflect::info::{Typed, Type};
/// use std::collections::BTreeSet;
///
/// let info = <BTreeSet<String> as Typed>::type_info().as_set().unwrap();
///
/// assert!(info.value_is::<String>());
/// ```
#[derive(Clone, Debug)]
pub struct SetInfo {
    ty: Type,
    generics: Generics,
    value_id: TypeId,
    // `TypeInfo` is created on first access; use a function pointer to delay it.
    value_info: fn() -> &'static TypeInfo,
    #[cfg(feature = "reflect_docs")]
    docs: Option<&'static str>,
}

impl SetInfo {
    impl_docs_fn!(docs);
    impl_type_fn!(ty);
    impl_generic_fn!(generics);

    /// Create a new [`SetInfo`].
    #[inline]
    pub const fn new<TSet: Set + TypePath, TValue: Reflect + Typed>() -> Self {
        Self {
            ty: Type::of::<TSet>(),
            generics: Generics::new(),
            value_id: TypeId::of::<TValue>(),
            value_info: TValue::type_info,
            #[cfg(feature = "reflect_docs")]
            docs: None,
        }
    }

    /// Returns the element [`Type`] of the set.
    #[inline]
    pub const fn value_id(&self) -> TypeId {
        self.value_id
    }

    /// Returns return if the value type is `T`.
    #[inline]
    pub fn value_is<T: Any>(&self) -> bool {
        self.value_id == TypeId::of::<T>()
    }

    /// Returns the value element's [`TypeInfo`].
    #[inline]
    pub fn value_info(&self) -> &'static TypeInfo {
        (self.value_info)()
    }
}
