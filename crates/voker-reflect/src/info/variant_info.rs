use alloc::boxed::Box;
use core::{error, fmt};

use voker_os::sync::Arc;

use crate::info::{CustomAttributes, NamedField, UnnamedField, impl_docs_fn};
use crate::info::{impl_custom_attributes_fn, impl_with_custom_attributes};

// -----------------------------------------------------------------------------
// Enum Variant Kind

/// Represents the kind/form of an enum variant.
///
/// # Kinds
///
/// - `A` -> Unit
/// - `A()` and `A(..)` -> Tuple
/// - `A{}` and `A{..}` -> Struct
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum VariantKind {
    Struct,
    Tuple,
    Unit,
}

impl fmt::Display for VariantKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Struct => f.pad("Struct"),
            Self::Tuple => f.pad("Tuple"),
            Self::Unit => f.pad("Unit"),
        }
    }
}

/// A [`VariantKind`]-specific error.
#[derive(Debug)]
pub struct VariantKindError {
    /// Expected variant type.
    expected: VariantKind,
    /// Received variant type.
    received: VariantKind,
}

impl fmt::Display for VariantKindError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "variant kind mismatch: expected {:?}, received {:?}",
            self.expected, self.received
        )
    }
}

impl error::Error for VariantKindError {}

// -----------------------------------------------------------------------------
// Struct-like variant

/// Information for struct-style enum variants.
///
/// # Examples
///
/// ```
/// # use voker_reflect::{Reflect, info::Typed};
/// #
/// #[derive(Reflect)]
/// enum MyEnum {
///   A {  // <-- struct variant
///     foo: usize
///   },
///   Other{ /* ... */ },
/// }
///
/// let info = MyEnum::type_info()
///     .as_enum().unwrap()
///     .variant("A").unwrap()
///     .as_struct_variant().unwrap();
///
/// assert_eq!(info.name(), "A");
/// assert_eq!(info.field_len(), 1);
/// ```
#[derive(Clone, Debug)]
pub struct StructVariantInfo {
    name: &'static str,
    // Usually, enum variants should not have too many fields.
    // So we use box slice to reduce type size, including `VariantInfo` size.
    fields: Box<[NamedField]>,
    field_names: Box<[&'static str]>,
    // Use `Option` to avoid allocating when there are no custom attributes.
    custom_attributes: Option<Arc<CustomAttributes>>,
    #[cfg(feature = "reflect_docs")]
    docs: Option<&'static str>,
}

impl StructVariantInfo {
    impl_docs_fn!(docs);
    impl_custom_attributes_fn!(custom_attributes);
    impl_with_custom_attributes!(custom_attributes);

    /// Create a new [`StructVariantInfo`].
    ///
    /// The order of internal fields is fixed, depends on the input order.
    pub fn new(name: &'static str, fields: &[NamedField]) -> Self {
        Self {
            name,
            fields: fields.to_vec().into_boxed_slice(),
            field_names: fields.iter().map(NamedField::name).collect(),
            custom_attributes: None,
            #[cfg(feature = "reflect_docs")]
            docs: None,
        }
    }

    /// Returns the name of this variant.
    #[inline]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns a slice containing the field names in declaration order.
    #[inline]
    pub fn field_names(&self) -> &[&'static str] {
        &self.field_names
    }

    /// Returns the [`NamedField`] for the given `name`, if present.
    pub fn field(&self, name: &str) -> Option<&NamedField> {
        self.fields.get(self.index_of(name)?)
    }

    /// Returns the [`NamedField`] at the given index, if present.
    #[inline]
    pub fn field_at(&self, index: usize) -> Option<&NamedField> {
        self.fields.get(index)
    }

    /// Returns the index for the given field `name`, if present.
    ///
    /// This is O(N) complexity.
    #[inline]
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.field_names.iter().position(|s| *s == name)
    }

    /// Returns an iterator over the fields in **declaration order**.
    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &NamedField> {
        self.fields.iter()
    }

    /// Returns the total number of fields in this variant.
    #[inline]
    pub fn field_len(&self) -> usize {
        self.fields.len()
    }
}

// -----------------------------------------------------------------------------
// Tuple-like variant

/// Information for tuple-style enum variants.
///
/// # Examples
///
/// ```
/// # use voker_reflect::{Reflect, info::Typed};
/// #
/// #[derive(Reflect)]
/// enum MyEnum {
///   A(usize),  // <-- tuple variant
///   Other{ /* ... */ },
/// }
///
/// let info = MyEnum::type_info()
///     .as_enum().unwrap()
///     .variant("A").unwrap()
///     .as_tuple_variant().unwrap();
///
/// assert_eq!(info.name(), "A");
/// assert_eq!(info.field_len(), 1);
/// ```
#[derive(Clone, Debug)]
pub struct TupleVariantInfo {
    name: &'static str,
    fields: Box<[UnnamedField]>,
    // Use `Option` to reduce unnecessary heap requests (when empty content).
    custom_attributes: Option<Arc<CustomAttributes>>,
    #[cfg(feature = "reflect_docs")]
    docs: Option<&'static str>,
}

impl TupleVariantInfo {
    impl_docs_fn!(docs);
    impl_custom_attributes_fn!(custom_attributes);
    impl_with_custom_attributes!(custom_attributes);

    /// Create a new [`TupleVariantInfo`].
    pub fn new(name: &'static str, fields: &[UnnamedField]) -> Self {
        // Not inline: Consistent with StructVariantInfo
        Self {
            name,
            fields: fields.to_vec().into_boxed_slice(),
            custom_attributes: None,
            #[cfg(feature = "reflect_docs")]
            docs: None,
        }
    }

    /// The name of this variant.
    #[inline]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Get the field at the given index.
    #[inline]
    pub fn field_at(&self, index: usize) -> Option<&UnnamedField> {
        self.fields.get(index)
    }

    /// Returns an iterator over the fields in **declaration order**.
    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &UnnamedField> {
        self.fields.iter()
    }

    /// The total number of fields in this variant.
    #[inline]
    pub fn field_len(&self) -> usize {
        self.fields.len()
    }
}

// -----------------------------------------------------------------------------
// Unit variant

/// Information for unit enum variants.
///
/// # Examples
///
/// ```
/// # use voker_reflect::{Reflect, info::Typed};
/// #
/// #[derive(Reflect)]
/// enum MyEnum {
///   A,  // <-- unit variant
///   Other{ /* ... */ },
/// }
///
/// let info = MyEnum::type_info()
///     .as_enum().unwrap()
///     .variant("A").unwrap()
///     .as_unit_variant().unwrap();
///
/// assert_eq!(info.name(), "A");
/// ```
#[derive(Clone, Debug)]
pub struct UnitVariantInfo {
    name: &'static str,
    // Use `Option` to reduce unnecessary heap requests (when empty content).
    custom_attributes: Option<Arc<CustomAttributes>>,
    #[cfg(feature = "reflect_docs")]
    docs: Option<&'static str>,
}

impl UnitVariantInfo {
    impl_docs_fn!(docs);
    impl_custom_attributes_fn!(custom_attributes);
    impl_with_custom_attributes!(custom_attributes);

    /// Create a new [`UnitVariantInfo`].
    #[inline]
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            custom_attributes: None,
            #[cfg(feature = "reflect_docs")]
            docs: None,
        }
    }

    /// The name of this variant.
    #[inline]
    pub const fn name(&self) -> &'static str {
        self.name
    }
}

// -----------------------------------------------------------------------------
// VariantInfo

/// Container for compile-time enum variant info.
///
/// # Examples
///
/// ```
/// use voker_reflect::{Reflect, info::{Typed, VariantKind}};
///
/// #[derive(Reflect)]
/// enum MyEnum {
///   A,
///   B(),
///   #[reflect(@10_i32)]
///   C{},
///   Other{ /* ... */ },
/// }
///
/// let enum_info = MyEnum::type_info().as_enum().unwrap();
///
/// let a = enum_info.variant("A").unwrap();
/// let b = enum_info.variant("B").unwrap();
/// let c = enum_info.variant("C").unwrap();
///
/// assert_eq!(a.name(), "A");
/// assert_eq!(b.variant_kind(), VariantKind::Tuple);
/// assert_eq!(c.get_attribute::<i32>(), Some(&10_i32));
/// ```
#[derive(Clone, Debug)]
pub enum VariantInfo {
    /// See [`StructVariantInfo`].
    Struct(StructVariantInfo),
    /// See [`TupleVariantInfo`].
    Tuple(TupleVariantInfo),
    /// See [`UnitVariantInfo`].
    Unit(UnitVariantInfo),
}

macro_rules! impl_cast_fn {
    ($name:ident : $kind:ident => $info:ident) => {
        pub fn $name(&self) -> Result<&$info, VariantKindError> {
            match self {
                Self::$kind(info) => Ok(info),
                _ => Err(VariantKindError {
                    expected: VariantKind::$kind,
                    received: self.variant_kind(),
                }),
            }
        }
    };
}

impl VariantInfo {
    impl_cast_fn!(as_struct_variant: Struct => StructVariantInfo);
    impl_cast_fn!(as_tuple_variant: Tuple => TupleVariantInfo);
    impl_cast_fn!(as_unit_variant: Unit => UnitVariantInfo);

    /// Returns the attribute of type T, if present.
    pub fn custom_attributes(&self) -> &CustomAttributes {
        match self {
            Self::Struct(info) => info.custom_attributes(),
            Self::Tuple(info) => info.custom_attributes(),
            Self::Unit(info) => info.custom_attributes(),
        }
    }

    impl_custom_attributes_fn!();

    /// The name of the enum variant.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Struct(info) => info.name(),
            Self::Tuple(info) => info.name(),
            Self::Unit(info) => info.name(),
        }
    }

    /// Returns the [`VariantKind`] of this variant.
    ///
    /// # Kinds
    ///
    /// - `A` -> Unit
    /// - `A()` and `A(..)` -> Tuple
    /// - `A{}` and `A{..}` -> Struct
    pub const fn variant_kind(&self) -> VariantKind {
        match self {
            Self::Struct(_) => VariantKind::Struct,
            Self::Tuple(_) => VariantKind::Tuple,
            Self::Unit(_) => VariantKind::Unit,
        }
    }

    /// Returns the number of fields in this variant.
    pub fn field_len(&self) -> usize {
        match self {
            Self::Struct(info) => info.field_len(),
            Self::Tuple(info) => info.field_len(),
            Self::Unit(_) => 0,
        }
    }

    /// The docstring of the underlying variant, if any.
    ///
    /// If `reflect_docs` feature is not enabled, this function always return `None`.
    /// So you can use this without worrying about compilation options.
    ///
    /// See examples in [`TypeInfo`](crate::info::TypeInfo) .
    #[cfg_attr(not(feature = "reflect_docs"), inline(always))]
    pub const fn docs(&self) -> Option<&str> {
        #[cfg(not(feature = "reflect_docs"))]
        return None;

        #[cfg(feature = "reflect_docs")]
        match self {
            Self::Struct(info) => info.docs(),
            Self::Tuple(info) => info.docs(),
            Self::Unit(info) => info.docs(),
        }
    }
}
