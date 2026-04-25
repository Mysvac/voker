use core::any::{Any, TypeId};

use alloc::sync::Arc;

use crate::info::{CustomAttributes, TypeInfo, Typed, impl_docs_fn};
use crate::info::{impl_custom_attributes_fn, impl_with_custom_attributes};

// -----------------------------------------------------------------------------
// NamedField

/// Information for a named (struct) field.
///
/// # Examples
///
/// ```
/// use voker_reflect::{Reflect, info::Typed};
///
/// #[derive(Reflect)]
/// struct Foo {
///     field_a: f32,
/// }
///
/// let info = Foo::type_info().as_struct().unwrap();
/// let field_info = info.field_at(0).unwrap();
///
/// assert!(field_info.type_is::<f32>());
/// assert_eq!(field_info.name(), "field_a");
/// ```
#[derive(Clone, Debug)]
pub struct NamedField {
    type_id: TypeId,
    name: &'static str,
    // `TypeInfo` is created on first access; using a function pointer delays it.
    type_info: fn() -> &'static TypeInfo,
    // Use `Option` to reduce unnecessary heap requests (when empty content).
    custom_attributes: Option<Arc<CustomAttributes>>,
    skip_serde: bool,
    #[cfg(feature = "reflect_docs")]
    docs: Option<&'static str>,
}

impl NamedField {
    impl_docs_fn!(docs);
    impl_custom_attributes_fn!(custom_attributes);
    impl_with_custom_attributes!(custom_attributes);

    /// Creates a new [`NamedField`] for the given field `name` and type `T`.
    #[inline]
    pub const fn new<T: Typed>(name: &'static str) -> Self {
        Self {
            name,
            type_info: T::type_info,
            type_id: TypeId::of::<T>(),
            custom_attributes: None,
            skip_serde: false,
            #[cfg(feature = "reflect_docs")]
            docs: None,
        }
    }

    /// Returns the `TypeId`.
    #[inline]
    pub const fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Check if the given type matches this one.
    #[inline]
    pub fn type_is<T: Any>(&self) -> bool {
        self.type_id == TypeId::of::<T>()
    }

    /// Returns the field name.
    #[inline]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the field's [`TypeInfo`].
    #[inline]
    pub fn type_info(&self) -> &'static TypeInfo {
        (self.type_info)()
    }

    /// Replaces stored skip_serde flag.
    #[inline]
    pub fn with_skip_serde(self, val: bool) -> Self {
        Self {
            skip_serde: val,
            ..self
        }
    }

    /// Checks whether the field should be skipped during
    /// (de)serialization, defaults to `false`.
    ///
    /// This flag only affects reflection-based (de)serialization.
    /// If a custom Serde implementation is provided by the user,
    /// this flag has no effect.
    ///
    /// When set to `true`, the corresponding type must be
    /// registered with [`ReflectDefault`] in the registry;
    /// otherwise, deserialization  will inevitably fail.
    ///
    /// [`ReflectDefault`]: crate::registry::ReflectDefault
    #[inline]
    pub const fn skip_serde(&self) -> bool {
        self.skip_serde
    }
}

// -----------------------------------------------------------------------------
// UnnamedField

/// Information for an unnamed (tuple) field, size = 40.
///
/// # Examples
///
/// ```
/// use voker_reflect::{Reflect, info::Typed};
///
/// #[derive(Reflect)]
/// struct Foo(f32);
///
/// let info = Foo::type_info().as_tuple_struct().unwrap();
/// let field_info = info.field(0).unwrap();
///
/// assert!(field_info.type_is::<f32>());
/// assert_eq!(field_info.index(), 0);
/// ```
#[derive(Clone, Debug)]
pub struct UnnamedField {
    type_id: TypeId,
    index: usize,
    // `TypeInfo` is created on first access; using a function pointer delays it.
    type_info: fn() -> &'static TypeInfo,
    // Use `Option` to reduce unnecessary heap requests (when empty content).
    custom_attributes: Option<Arc<CustomAttributes>>,
    skip_serde: bool,
    #[cfg(feature = "reflect_docs")]
    docs: Option<&'static str>,
}

impl UnnamedField {
    impl_docs_fn!(docs);
    impl_custom_attributes_fn!(custom_attributes);
    impl_with_custom_attributes!(custom_attributes);

    /// Creates a new [`UnnamedField`] for the field at `index` with type `T`.
    #[inline]
    pub const fn new<T: Typed>(index: usize) -> Self {
        Self {
            index,
            type_info: T::type_info,
            type_id: TypeId::of::<T>(),
            custom_attributes: None,
            skip_serde: false,
            #[cfg(feature = "reflect_docs")]
            docs: None,
        }
    }

    /// Returns the `TypeId`.
    #[inline]
    pub const fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Check if the given type matches this one.
    #[inline]
    pub fn type_is<T: Any>(&self) -> bool {
        self.type_id == TypeId::of::<T>()
    }

    /// Returns the field index (position in the tuple struct).
    #[inline]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// Returns the field's [`TypeInfo`].
    #[inline]
    pub fn type_info(&self) -> &'static TypeInfo {
        (self.type_info)()
    }

    /// Replaces stored skip_serde flag.
    #[inline]
    pub fn with_skip_serde(self, val: bool) -> Self {
        Self {
            skip_serde: val,
            ..self
        }
    }

    /// Checks whether the field should be skipped during (de)serialization, defaults to `false`.
    ///
    /// This flag only affects reflection-based (de)serialization.
    /// If a custom Serde implementation is provided by the user,
    /// this flag has no effect.
    ///
    /// When set to `true`, the corresponding type must be
    /// registered with [`ReflectDefault`] in the registry;
    /// otherwise, deserialization  will inevitably fail.
    ///
    /// [`ReflectDefault`]: crate::registry::ReflectDefault
    #[inline]
    pub const fn skip_serde(&self) -> bool {
        self.skip_serde
    }
}
