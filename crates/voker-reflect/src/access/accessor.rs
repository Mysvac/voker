//! Provide single-layer path accessing support

use alloc::borrow::Cow;
use core::fmt;

use crate::Reflect;
use crate::info::{ReflectKind, VariantKind};
use crate::ops::{ReflectMut, ReflectRef};

// -----------------------------------------------------------------------------
// Single layer accessor

/// A **singular** element access within a path.
///
/// A fundamental component of path access,
/// supported for [`Struct`], [`TupleStruct`], [`Tuple`], [`Array`], [`List`], [`Enum`].
///
/// # Rules
///
/// - FieldName: Can be used to access struct or enum's struct variant.
/// - FieldIndex: Can be used to access struct or enum's struct variant.
/// - TupleIndex: Can be used to access tuple, tuple-struct or enum's tuple variant.
/// - ListIndex: Can be used to access list and array.
///
/// # Examples
///
/// ```
/// use voker_reflect::{Reflect, access::Accessor};
///
/// #[derive(Reflect)]
/// struct Foo {
///     a: i32,
///     b: bool,
/// }
///
/// let foo = Foo{ a: 11, b: true };
///
/// // Field Name
/// let accessor = Accessor::FieldName("a".into());
/// let elem = accessor.access(&foo, None).unwrap().downcast_ref::<i32>().unwrap();
/// assert_eq!(*elem, 11);
///
/// // Field Index
/// let accessor = Accessor::FieldIndex(1);
/// let elem = accessor.access(&foo, None).unwrap().downcast_ref::<bool>().unwrap();
/// assert_eq!(*elem, true);
///
/// // Tuple Index
/// let arr = (10, true, "hello");
/// let accessor = Accessor::TupleIndex(1);
/// let elem = accessor.access(&arr, None).unwrap().downcast_ref::<bool>().unwrap();
/// assert_eq!(*elem, true);
///
/// // List Index
/// let arr = [10, 20, 30, 40];
/// let accessor = Accessor::ListIndex(1);
/// let elem = accessor.access(&arr, None).unwrap().downcast_ref::<i32>().unwrap();
/// assert_eq!(*elem, 20);
/// ```
///
/// [`Struct`]: crate::ops::Struct
/// [`TupleStruct`]: crate::ops::TupleStruct
/// [`Tuple`]: crate::ops::Tuple
/// [`Array`]: crate::ops::Array
/// [`List`]: crate::ops::List
/// [`Enum`]: crate::ops::Enum
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Accessor<'a> {
    /// A name-based field access on a struct or enum struct.
    ///
    /// Example: the `id` of `.id` (default impl)
    FieldName(Cow<'a, str>),
    /// An index-based field access on a tuple, tuple struct, or enum tuple.
    ///
    /// Example: the `5` of `.5` (default impl)
    TupleIndex(usize),
    /// An index-based access on a list or array.
    ///
    /// Example: the `5` of `[5]` (default impl)
    ListIndex(usize),
    /// An index-based field access on a struct or enum struct.
    ///
    /// Can only be used to access a struct (excluding tuple structs).
    ///
    /// Example: the `5` of `"#5"` (default impl)
    FieldIndex(usize),
}

// -----------------------------------------------------------------------------
// Error

/// The kind of [`AccessError`], along with some kind-specific information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessErrorKind {
    MissingField(ReflectKind),
    IncompatibleKinds {
        expected: ReflectKind,
        actual: ReflectKind,
    },
    IncompatibleVariantKinds {
        expected: VariantKind,
        actual: VariantKind,
    },
}

/// An error originating from an [`Accessor`] of an element within a type.
///
/// Use the `Display` impl of this type to get information on the error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessError<'a> {
    kind: AccessErrorKind,
    accessor: Accessor<'a>,
    offset: Option<usize>,
}

impl fmt::Display for Accessor<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Accessor::FieldName(field) => write!(f, ".{field}"),
            Accessor::FieldIndex(index) => write!(f, "#{index}"),
            Accessor::TupleIndex(index) => write!(f, ".{index}"),
            Accessor::ListIndex(index) => write!(f, "[{index}]"),
        }
    }
}

macro_rules! invalid_kind {
    ($expected:path, $actual:expr) => {
        AccessErrorKind::IncompatibleKinds {
            expected: $expected,
            actual: $actual,
        }
    };
}

macro_rules! match_variant {
    ($name:ident, $kind:path => $expr:expr) => {
        match $name.variant_kind() {
            $kind => Ok($expr),
            actual => Err(AccessErrorKind::IncompatibleVariantKinds {
                expected: $kind,
                actual,
            }),
        }
    };
}

// -----------------------------------------------------------------------------
// Accessor implementation

impl<'a> Accessor<'a> {
    /// Converts this into an "owned" value.
    #[inline]
    pub fn into_owned(self) -> Accessor<'static> {
        match self {
            Self::FieldName(value) => Accessor::FieldName(Cow::Owned(value.into_owned())),
            Self::FieldIndex(value) => Accessor::FieldIndex(value),
            Self::TupleIndex(value) => Accessor::TupleIndex(value),
            Self::ListIndex(value) => Accessor::ListIndex(value),
        }
    }

    // Returns a reference to  inner value as a `&dyn Display`
    fn display_value(&self) -> &dyn fmt::Display {
        match self {
            Self::FieldName(value) => value,
            Self::FieldIndex(value) | Self::TupleIndex(value) | Self::ListIndex(value) => value,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::FieldName(_) => "FieldName",
            Self::FieldIndex(_) => "FieldIndex",
            Self::TupleIndex(_) => "TupleIndex",
            Self::ListIndex(_) => "ListIndex",
        }
    }

    /// Dynamically accesses a field; on success returns a shared reference.
    pub fn access<'r>(
        &self,
        base: &'r dyn Reflect,
        offset: Option<usize>, // use for error info
    ) -> Result<&'r dyn Reflect, AccessError<'a>> {
        use ReflectRef::*;

        let res: Result<Option<&'r dyn Reflect>, AccessErrorKind> = match (self, base.reflect_ref())
        {
            (Self::FieldName(field), Struct(struct_ref)) => Ok(struct_ref.field(field.as_ref())),
            (Self::FieldName(field), Enum(enum_ref)) => {
                match_variant!(enum_ref, VariantKind::Struct => enum_ref.field(field.as_ref()))
            }
            (Self::FieldName(_), actual) => Err(invalid_kind!(ReflectKind::Struct, actual.kind())),
            (&Self::FieldIndex(index), Struct(struct_ref)) => Ok(struct_ref.field_at(index)),
            (&Self::FieldIndex(index), Enum(enum_ref)) => {
                match_variant!(enum_ref, VariantKind::Struct => enum_ref.field_at(index))
            }
            (Self::FieldIndex(_), actual) => Err(invalid_kind!(ReflectKind::Struct, actual.kind())),
            (&Self::TupleIndex(index), TupleStruct(tuple)) => Ok(tuple.field(index)),
            (&Self::TupleIndex(index), Tuple(tuple)) => Ok(tuple.field(index)),
            (&Self::TupleIndex(index), Enum(enum_ref)) => {
                match_variant!(enum_ref, VariantKind::Tuple => enum_ref.field_at(index))
            }
            (Self::TupleIndex(_), actual) => Err(invalid_kind!(ReflectKind::Tuple, actual.kind())),
            (&Self::ListIndex(index), List(list)) => Ok(list.get(index)),
            (&Self::ListIndex(index), Array(list)) => Ok(list.get(index)),
            (Self::ListIndex(_), actual) => Err(invalid_kind!(ReflectKind::List, actual.kind())),
        };

        res.and_then(|opt| opt.ok_or(AccessErrorKind::MissingField(base.reflect_kind())))
            .map_err(|kind| AccessError {
                kind,
                accessor: self.clone(),
                offset,
            })
    }

    /// Dynamically accesses a field; on success returns a mutable reference.
    pub fn access_mut<'r>(
        &self,
        base: &'r mut dyn Reflect,
        offset: Option<usize>, // use for error info
    ) -> Result<&'r mut dyn Reflect, AccessError<'a>> {
        use ReflectMut::*;

        let base_kind = base.reflect_kind();

        let res: Result<Option<&'r mut dyn Reflect>, AccessErrorKind> = match (
            self,
            base.reflect_mut(),
        ) {
            (Self::FieldName(field), Struct(struct_mut)) => {
                Ok(struct_mut.field_mut(field.as_ref()))
            }
            (Self::FieldName(field), Enum(enum_mut)) => {
                match_variant!(enum_mut, VariantKind::Struct => enum_mut.field_mut(field.as_ref()))
            }
            (Self::FieldName(_), actual) => Err(invalid_kind!(ReflectKind::Struct, actual.kind())),
            (&Self::FieldIndex(index), Struct(struct_mut)) => Ok(struct_mut.field_at_mut(index)),
            (&Self::FieldIndex(index), Enum(enum_mut)) => {
                match_variant!(enum_mut, VariantKind::Struct => enum_mut.field_at_mut(index))
            }
            (Self::FieldIndex(_), actual) => Err(invalid_kind!(ReflectKind::Struct, actual.kind())),
            (&Self::TupleIndex(index), TupleStruct(tuple)) => Ok(tuple.field_mut(index)),
            (&Self::TupleIndex(index), Tuple(tuple)) => Ok(tuple.field_mut(index)),
            (&Self::TupleIndex(index), Enum(enum_mut)) => {
                match_variant!(enum_mut, VariantKind::Tuple => enum_mut.field_at_mut(index))
            }
            (Self::TupleIndex(_), actual) => Err(invalid_kind!(ReflectKind::Tuple, actual.kind())),
            (&Self::ListIndex(index), List(list)) => Ok(list.get_mut(index)),
            (&Self::ListIndex(index), Array(list)) => Ok(list.get_mut(index)),
            (Self::ListIndex(_), actual) => Err(invalid_kind!(ReflectKind::List, actual.kind())),
        };

        res.and_then(|opt| opt.ok_or(AccessErrorKind::MissingField(base_kind)))
            .map_err(|kind| AccessError {
                kind,
                accessor: self.clone(),
                offset,
            })
    }
}

// -----------------------------------------------------------------------------
// Error implementation

impl<'a> AccessError<'a> {
    /// Returns the kind of [`AccessError`].
    #[inline]
    pub fn kind(&self) -> &AccessErrorKind {
        &self.kind
    }

    /// Returns the [`Accessor`] that this [`AccessError`] occurred in.
    #[inline]
    pub fn accessor(&self) -> &Accessor<'_> {
        &self.accessor
    }

    /// If the [`Accessor`] was created with a parser or an offset was manually provided,
    /// returns the offset of the [`Accessor`] in its path string.
    #[inline]
    pub fn offset(&self) -> Option<&usize> {
        self.offset.as_ref()
    }
}

impl<'a> fmt::Display for AccessError<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let AccessError {
            kind,
            accessor,
            offset,
        } = self;

        write!(f, "Error accessing element with `{accessor}` accessor")?;
        if let Some(offset) = offset {
            write!(f, "(offset {offset})")?;
        }
        write!(f, ": ")?;

        match kind {
            AccessErrorKind::MissingField(type_accessed) => match accessor {
                Accessor::FieldName(_) => write!(
                    f,
                    "The {type_accessed} accessed doesn't have field name `{}`",
                    accessor.display_value()
                ),
                Accessor::FieldIndex(_) => write!(
                    f,
                    "The {type_accessed} accessed doesn't have field index `{}`",
                    accessor.display_value(),
                ),
                Accessor::TupleIndex(_) | Accessor::ListIndex(_) => write!(
                    f,
                    "The {type_accessed} accessed doesn't have index `{}`",
                    accessor.display_value()
                ),
            },
            AccessErrorKind::IncompatibleKinds { expected, actual } => write!(
                f,
                "Expected {} accessor to access a {expected}, found a {actual} instead.",
                accessor.kind()
            ),
            AccessErrorKind::IncompatibleVariantKinds { expected, actual } => write!(
                f,
                "Expected variant {} accessor to access a {expected} variant, found a {actual} variant instead.",
                accessor.kind()
            ),
        }
    }
}

impl core::error::Error for AccessError<'_> {}

// -----------------------------------------------------------------------------
// Single layer accessor with offset

/// An [`Accessor`] combined with an `offset` for more helpful error reporting.
///
/// `offset` is only used to display error messages, unrelated to access.
///
/// # Examples
///
/// ```
/// use voker_reflect::{Reflect, access::{Accessor, OffsetAccessor}};
///
/// #[derive(Reflect)]
/// struct Foo {
///     a: i32,
///     b: bool,
/// }
///
/// let foo = Foo{ a: 11, b: true };
///
/// // Field Name
/// let accessor = OffsetAccessor::from(Accessor::FieldName("a".into()));
/// let elem = accessor.access(&foo).unwrap().downcast_ref::<i32>().unwrap();
/// assert_eq!(*elem, 11);
///
/// // Field Index
/// let accessor = OffsetAccessor::from(Accessor::FieldIndex(1));
/// let elem = accessor.access(&foo).unwrap().downcast_ref::<bool>().unwrap();
/// assert_eq!(*elem, true);
///
/// // Tuple Index
/// let arr = (10, true, "hello");
/// let accessor = OffsetAccessor::from(Accessor::TupleIndex(1));
/// let elem = accessor.access(&arr).unwrap().downcast_ref::<bool>().unwrap();
/// assert_eq!(*elem, true);
///
/// // List Index
/// let arr = [10, 20, 30, 40];
/// let accessor = OffsetAccessor::from(Accessor::ListIndex(1));
/// let elem = accessor.access(&arr).unwrap().downcast_ref::<i32>().unwrap();
/// assert_eq!(*elem, 20);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OffsetAccessor<'a> {
    pub accessor: Accessor<'a>,
    /// only used to display error messages
    pub offset: Option<usize>,
}

impl<'a> From<Accessor<'a>> for OffsetAccessor<'a> {
    #[inline]
    fn from(accessor: Accessor<'a>) -> Self {
        Self {
            accessor,
            offset: None,
        }
    }
}

impl<'a> OffsetAccessor<'a> {
    /// Converts this into an "owned" value.
    #[inline]
    pub fn into_owned(self) -> OffsetAccessor<'static> {
        OffsetAccessor {
            accessor: self.accessor.into_owned(),
            offset: self.offset,
        }
    }

    /// Dynamically accesses a field; on success returns a shared reference.
    #[inline]
    pub fn access<'r>(&self, base: &'r dyn Reflect) -> Result<&'r dyn Reflect, AccessError<'a>> {
        self.accessor.access(base, self.offset)
    }

    /// Dynamically accesses a field; on success returns a mutable reference.
    #[inline]
    pub fn access_mut<'r>(
        &self,
        base: &'r mut dyn Reflect,
    ) -> Result<&'r mut dyn Reflect, AccessError<'a>> {
        self.accessor.access_mut(base, self.offset)
    }
}
