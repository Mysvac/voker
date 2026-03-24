use core::{error, fmt};

use crate::info::{ArrayInfo, ListInfo, TupleInfo};
use crate::info::{CustomAttributes, Generics, Type};
use crate::info::{EnumInfo, StructInfo, TupleStructInfo};
use crate::info::{MapInfo, OpaqueInfo, SetInfo};

// -----------------------------------------------------------------------------
// ReflectKind

/// An enumeration of the "kinds" of a reflected type.
///
/// Each kind corresponds to a specific reflection trait,
/// such as `Struct` or `List`, which itself corresponds
/// to the kind or structure of a type.
///
/// A [`ReflectKind`] is obtained via [`Reflect::reflect_kind`],
/// or via [`ReflectRef::kind`], [`ReflectMut::kind`] and [`ReflectOwned::kind`].
///
/// [`Reflect::reflect_kind`]: crate::Reflect::reflect_kind
/// [`ReflectRef::kind`]: crate::ops::ReflectRef
/// [`ReflectMut::kind`]: crate::ops::ReflectMut
/// [`ReflectOwned::kind`]: crate::ops::ReflectOwned
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReflectKind {
    Struct,
    TupleStruct,
    Tuple,
    List,
    Array,
    Map,
    Set,
    Enum,
    Opaque,
}

impl fmt::Display for ReflectKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Struct => f.pad("Struct"),
            Self::TupleStruct => f.pad("TupleStruct"),
            Self::Tuple => f.pad("Tuple"),
            Self::List => f.pad("List"),
            Self::Array => f.pad("Array"),
            Self::Map => f.pad("Map"),
            Self::Set => f.pad("Set"),
            Self::Enum => f.pad("Enum"),
            Self::Opaque => f.pad("Opaque"),
        }
    }
}

/// Error returned when a `TypeInfo` value is not the expected `ReflectKind`.
#[derive(Debug)]
pub struct ReflectKindError {
    pub expected: ReflectKind,
    pub received: ReflectKind,
}

impl fmt::Display for ReflectKindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "reflect kind mismatch: expected {}, received {}",
            self.expected, self.received
        )
    }
}

impl error::Error for ReflectKindError {}

// -----------------------------------------------------------------------------
// TypeInfo

/// Compile-time type information for various reflected types.
///
/// # Content
///
/// A `TypeInfo` contains the following information:
///
/// - **kind**: The [`ReflectKind`], such as `Struct` or `Enum`.
/// - **id**: The unique type identifier, [`core::any::TypeId`].
/// - **name**: Type naming information, as provided by [`TypePathTable`].
/// - **generics**: [`Generics`], including type and const parameters.
/// - **attributes**: [`CustomAttributes`], similar to C# attributes.
/// - **docs**: Type documentation, available with the `reflect_docs` feature.
///
/// It can also be converted into more specific metadata types, such as [`StructInfo`]
/// and [`EnumInfo`], which expose field and variant details.
///
/// # Obtain
///
/// In general, a type's `TypeInfo` is provided by the [`Typed`] trait.
///
/// For any given type, it can be retrieved in one of four ways:
///
/// 1. [`Typed::type_info`]
/// 2. [`DynamicTyped::reflect_type_info`]
/// 3. [`Reflect::represented_type_info`]
/// 4. [`TypeRegistry::get_type_info`]
///
/// Each returns a static reference to [`TypeInfo`], but they all have their own use cases.
///
/// - If you know the type at compile time, [`Typed::type_info`] is probably the simplest.
/// - If you have a `dyn Reflect`, use [`DynamicTyped::reflect_type_info`].
/// - If you only care about data content (such as serialization), then [`Reflect::represented_type_info`] should be used.
/// - If all you have is a [`TypeId`] or [type path], use [`TypeRegistry::get_type_info`].
///
/// You may also opt to use [`TypeRegistry::get_type_info`] in place of the other methods simply because
/// it can be more performant. This is because those other methods may require attaining a lock on
/// the static [`TypeInfo`], while the registry simply checks a map.
///
/// [`Typed`]: crate::info::Typed
/// [`TypeId`]: core::any::TypeId
/// [`TypePathTable`]: crate::info::TypePathTable
/// [type path]: crate::info::TypePath
/// [`Typed::type_info`]: crate::info::Typed::type_info
/// [`DynamicTyped::reflect_type_info`]: crate::info::DynamicTyped::reflect_type_info
/// [`Reflect::represented_type_info`]: crate::Reflect::represented_type_info
/// [`TypeRegistry::get_type_info`]: crate::registry::TypeRegistry::get_type_info
#[derive(Debug, Clone)]
pub enum TypeInfo {
    Struct(StructInfo),
    TupleStruct(TupleStructInfo),
    Tuple(TupleInfo),
    List(ListInfo),
    Array(ArrayInfo),
    Map(MapInfo),
    Set(SetInfo),
    Enum(EnumInfo),
    Opaque(OpaqueInfo),
}

// Helper macro that implements type-safe accessor methods like `as_struct`.
macro_rules! impl_cast_method {
    ($name:ident : $kind:ident => $info:ident) => {
        /// Convert [`TypeInfo`] to specific type information.
        ///
        /// Then you can call some more specific methods,
        /// and methods such as `ty` and `custom_attributes` will also be more efficient,
        /// without the need to determine the [type kind](ReflectKind).
        pub const fn $name(&self) -> Result<&$info, ReflectKindError> {
            match self {
                Self::$kind(info) => Ok(info),
                _ => Err(ReflectKindError {
                    expected: ReflectKind::$kind,
                    received: self.kind(),
                }),
            }
        }
    };
}

macro_rules! impl_is_method {
    ($name:ident : $kind:ident) => {
        /// Checks the metadata kind. Can be used in const functions.
        #[inline]
        pub(crate) const fn $name(&self) -> bool {
            match self {
                Self::$kind(..) => true,
                _ => false,
            }
        }
    };
}

impl TypeInfo {
    impl_cast_method!(as_struct: Struct => StructInfo);
    impl_cast_method!(as_tuple_struct: TupleStruct => TupleStructInfo);
    impl_cast_method!(as_tuple: Tuple => TupleInfo);
    impl_cast_method!(as_list: List => ListInfo);
    impl_cast_method!(as_array: Array => ArrayInfo);
    impl_cast_method!(as_map: Map => MapInfo);
    impl_cast_method!(as_set: Set => SetInfo);
    impl_cast_method!(as_enum: Enum => EnumInfo);
    impl_cast_method!(as_opaque: Opaque => OpaqueInfo);

    impl_is_method!(is_struct: Struct);
    impl_is_method!(is_tuple_struct: TupleStruct);
    impl_is_method!(is_tuple: Tuple);
    impl_is_method!(is_list: List);
    impl_is_method!(is_array: Array);
    impl_is_method!(is_map: Map);
    impl_is_method!(is_set: Set);
    impl_is_method!(is_enum: Enum);

    /// Returns the underlying [`Type`] metadata for this `TypeInfo`.
    pub const fn ty(&self) -> &Type {
        match self {
            Self::Struct(info) => info.ty(),
            Self::TupleStruct(info) => info.ty(),
            Self::Tuple(info) => info.ty(),
            Self::List(info) => info.ty(),
            Self::Array(info) => info.ty(),
            Self::Map(info) => info.ty(),
            Self::Set(info) => info.ty(),
            Self::Enum(info) => info.ty(),
            Self::Opaque(info) => info.ty(),
        }
    }

    crate::info::impl_type_fn!();

    /// Returns the [`ReflectKind`] for this `TypeInfo` (a fast discriminator).
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::info::{Typed, ReflectKind};
    ///
    /// let info = i32::type_info();
    /// assert_eq!(info.kind(), ReflectKind::Opaque);
    /// ```
    pub const fn kind(&self) -> ReflectKind {
        match self {
            Self::Struct(_) => ReflectKind::Struct,
            Self::TupleStruct(_) => ReflectKind::TupleStruct,
            Self::Tuple(_) => ReflectKind::Tuple,
            Self::List(_) => ReflectKind::List,
            Self::Array(_) => ReflectKind::Array,
            Self::Map(_) => ReflectKind::Map,
            Self::Set(_) => ReflectKind::Set,
            Self::Enum(_) => ReflectKind::Enum,
            Self::Opaque(_) => ReflectKind::Opaque,
        }
    }

    /// Returns the generics metadata (type/const parameters) for this type.
    ///
    /// Note: this is not inlined to avoid recursive inline expansion across
    /// `TypeInfo` variants.
    ///
    /// See examples in [`Generics`](crate::info::Generics) .
    pub const fn generics(&self) -> &Generics {
        match self {
            Self::Struct(info) => info.generics(),
            Self::TupleStruct(info) => info.generics(),
            Self::Tuple(info) => info.generics(),
            Self::List(info) => info.generics(),
            Self::Array(info) => info.generics(),
            Self::Map(info) => info.generics(),
            Self::Set(info) => info.generics(),
            Self::Enum(info) => info.generics(),
            Self::Opaque(info) => info.generics(),
        }
    }

    /// Returns the custom attributes attached to this type, if any.
    ///
    /// For kinds that do not support custom attributes this returns a shared
    /// empty reference (`CustomAttributes::EMPTY`).
    ///
    /// See examples in [`CustomAttributes`](crate::info::CustomAttributes) .
    pub fn custom_attributes(&self) -> &CustomAttributes {
        match self {
            Self::Struct(info) => info.custom_attributes(),
            Self::TupleStruct(info) => info.custom_attributes(),
            Self::Enum(info) => info.custom_attributes(),
            Self::Opaque(info) => info.custom_attributes(),
            _ => CustomAttributes::EMPTY,
        }
    }
    crate::info::attributes::impl_custom_attributes_fn!();

    /// Returns the documentation string for the type, if `reflect_docs` is
    /// enabled and docs are present.
    ///
    /// If `reflect_docs` feature is not enabled, this function always return `None`.
    /// So you can use this without worrying about compilation options.
    ///
    ///
    /// # Examples
    ///
    /// If `reflect_docs` feature is enabled, `Reflect` macro will collect
    /// `#[doc = "..."]` attibutes, including `///` syntax.
    ///
    /// ```
    /// # use voker_reflect::{Reflect, info::Typed};
    /// #
    /// /// This is type A.
    /// #[derive(Reflect)]
    /// struct A;
    ///
    /// let info = A::type_info();
    ///
    /// if let Some(docs) = info.docs() {
    ///     println!("{docs}"); // "This is type A."
    /// } else {
    ///     println!("`reflect_docs` is disabled or document is empty.");
    /// }
    /// ```
    ///
    /// If you do not want to use standard documents (customization is required),
    /// please use syntax `#[reflect(doc = "...")]`.
    ///
    /// ```
    /// # use voker_reflect::{Reflect, info::Typed};
    /// #
    /// /// This is type A.
    /// #[derive(Reflect)]
    /// #[reflect(doc = "This is type B")]
    /// struct A;
    ///
    /// let info = A::type_info();
    ///
    /// if let Some(docs) = info.docs() {
    ///     println!("{docs}"); // "This is type B."
    /// } else {
    ///     println!("`reflect_docs` is disabled or document is empty.");
    /// }
    /// ```
    ///
    /// If you want to close the document when `reflect_docs` feature is enabled,
    /// please use the `#[reflect(doc = false)]` tag.
    ///
    /// ```
    /// # use voker_reflect::{Reflect, info::Typed};
    /// #
    /// /// This is type A.
    /// #[derive(Reflect)]
    /// #[reflect(doc = "This is type B")]
    /// #[reflect(doc = false)]
    /// struct A;
    ///
    /// let info = A::type_info();
    /// assert!(info.docs().is_none());
    /// ```
    #[cfg_attr(not(feature = "reflect_docs"), inline(always))]
    pub const fn docs(&self) -> Option<&str> {
        #[cfg(not(feature = "reflect_docs"))]
        return None;
        #[cfg(feature = "reflect_docs")]
        match self {
            Self::Struct(info) => info.docs(),
            Self::TupleStruct(info) => info.docs(),
            Self::Tuple(info) => info.docs(),
            Self::List(info) => info.docs(),
            Self::Array(info) => info.docs(),
            Self::Map(info) => info.docs(),
            Self::Set(info) => info.docs(),
            Self::Enum(info) => info.docs(),
            Self::Opaque(info) => info.docs(),
        }
    }
}
