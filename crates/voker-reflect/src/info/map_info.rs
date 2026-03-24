use core::any::{Any, TypeId};

use crate::Reflect;
use crate::info::{Generics, Type, TypeInfo, TypePath, Typed};
use crate::info::{impl_docs_fn, impl_generic_fn, impl_type_fn};
use crate::ops::Map;

/// A container for compile-time map-like info, size = 112 (exclude `docs`).
///
/// At present, `MapInfo` does not have `CustomAttributes`, which can save memory.
///
/// # Examples
///
/// ```rust
/// # use core::any::TypeId;
/// use voker_reflect::info::{Typed, Type};
/// use std::collections::BTreeMap;
///
/// let info = <BTreeMap<String, i32> as Typed>::type_info().as_map().unwrap();
///
/// assert_eq!(info.key_id(), TypeId::of::<String>());
/// assert_eq!(info.value_id(), TypeId::of::<i32>());
/// ```
#[derive(Clone, Debug)]
pub struct MapInfo {
    ty: Type,
    generics: Generics,
    // Cache type_id for deserialization.
    // We don't have cache `Type` because it's too large.
    // But you can obtain `Type` through `TypeInfo`.
    key_id: TypeId,
    value_id: TypeId,
    // `TypeInfo` is created on first access; use function pointers to delay it.
    key_info: fn() -> &'static TypeInfo,
    value_info: fn() -> &'static TypeInfo,
    #[cfg(feature = "reflect_docs")]
    docs: Option<&'static str>,
}

impl MapInfo {
    impl_docs_fn!(docs);
    impl_type_fn!(ty);
    impl_generic_fn!(generics);

    /// Create a new [`MapInfo`].
    #[inline]
    pub const fn new<TMap: Map + TypePath, TKey: Reflect + Typed, TValue: Reflect + Typed>() -> Self
    {
        Self {
            ty: Type::of::<TMap>(),
            generics: Generics::new(),
            key_id: TypeId::of::<TKey>(),
            value_id: TypeId::of::<TValue>(),
            key_info: TKey::type_info,
            value_info: TValue::type_info,
            #[cfg(feature = "reflect_docs")]
            docs: None,
        }
    }

    /// Returns the [`Type`] of the key.
    #[inline]
    pub const fn key_id(&self) -> TypeId {
        self.key_id
    }

    /// Returns return if the key type is `T`.
    #[inline]
    pub fn key_is<T: Any>(&self) -> bool {
        self.key_id == TypeId::of::<T>()
    }

    /// Returns the [`Type`] of the value.
    #[inline]
    pub const fn value_id(&self) -> TypeId {
        self.value_id
    }

    /// Returns return if the value type is `T`.
    #[inline]
    pub fn value_is<T: Any>(&self) -> bool {
        self.value_id == TypeId::of::<T>()
    }

    /// Returns the key's [`TypeInfo`].
    #[inline]
    pub fn key_info(&self) -> &'static TypeInfo {
        (self.key_info)()
    }

    /// Returns the value's [`TypeInfo`].
    #[inline]
    pub fn value_info(&self) -> &'static TypeInfo {
        (self.value_info)()
    }
}
