use alloc::boxed::Box;

use crate::Reflect;
use crate::info::{ReflectKind, ReflectKindError};
use crate::ops::{Array, Enum, List, Map, Set, Struct, Tuple, TupleStruct};

// -----------------------------------------------------------------------------
// ReflectRef

/// An immutable enumeration of ["kinds"](ReflectKind) of a reflected type.
///
/// Each variant contains a trait object with methods specific to a kind of
/// type.
///
/// A [`ReflectRef`] is obtained via [`Reflect::reflect_ref`],
/// its kind must be consistent with [`Reflect::reflect_kind`].
///
/// # Examples
///
/// ```
/// # use voker_reflect::{Reflect, ops::Enum};
/// let p = Reflect::into_boxed_reflect(Some(true));
///
/// let dyn_enum: &dyn Enum = p.reflect_ref().as_enum().unwrap();
/// ```
pub enum ReflectRef<'a> {
    Struct(&'a dyn Struct),
    TupleStruct(&'a dyn TupleStruct),
    Tuple(&'a dyn Tuple),
    List(&'a dyn List),
    Array(&'a dyn Array),
    Map(&'a dyn Map),
    Set(&'a dyn Set),
    Enum(&'a dyn Enum),
    Opaque(&'a dyn Reflect),
}

// -----------------------------------------------------------------------------
// ReflectMut

/// A mutable enumeration of ["kinds"](ReflectKind) of a reflected type.
///
/// Each variant contains a trait object with methods specific to a kind of
/// type.
///
/// A [`ReflectMut`] is obtained via [`Reflect::reflect_mut`],
/// its kind must be consistent with [`Reflect::reflect_kind`].
///
/// # Examples
///
/// ```
/// # use voker_reflect::{Reflect, ops::Enum};
/// let mut p = Reflect::into_boxed_reflect(Some(true));
///
/// let dyn_enum: &mut dyn Enum = p.reflect_mut().as_enum().unwrap();
/// ```
pub enum ReflectMut<'a> {
    Struct(&'a mut dyn Struct),
    TupleStruct(&'a mut dyn TupleStruct),
    Tuple(&'a mut dyn Tuple),
    List(&'a mut dyn List),
    Array(&'a mut dyn Array),
    Map(&'a mut dyn Map),
    Set(&'a mut dyn Set),
    Enum(&'a mut dyn Enum),
    Opaque(&'a mut dyn Reflect),
}

// -----------------------------------------------------------------------------
// ReflectOwned

/// An owned enumeration of ["kinds"](ReflectKind) of a reflected type.
///
/// Each variant contains a trait object with methods specific to a kind of
/// type.
///
/// A [`ReflectOwned`] is obtained via [`Reflect::reflect_owned`],
/// its kind must be consistent with [`Reflect::reflect_kind`].
///
/// # Examples
///
/// ```
/// # use voker_reflect::{Reflect, ops::Enum};
/// let p = Reflect::into_boxed_reflect(Some(true));
///
/// let dyn_enum: Box<dyn Enum> = p.reflect_owned().into_enum().unwrap();
/// ```
pub enum ReflectOwned {
    Struct(Box<dyn Struct>),
    TupleStruct(Box<dyn TupleStruct>),
    Tuple(Box<dyn Tuple>),
    List(Box<dyn List>),
    Array(Box<dyn Array>),
    Map(Box<dyn Map>),
    Set(Box<dyn Set>),
    Enum(Box<dyn Enum>),
    Opaque(Box<dyn Reflect>),
}

// -----------------------------------------------------------------------------
// Implementation

macro_rules! impl_kind_fn {
    () => {
        /// Returns the [`ReflectKind`] of this reflected type without any information.
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
    };
}

macro_rules! impl_cast_fn {
    ($name:ident : $kind:ident => $retval:ty) => {
        #[doc = concat!("Attempts a cast to a `", stringify!($kind), "` trait object.")]
        pub fn $name(self) -> Result<$retval, ReflectKindError> {
            match self {
                Self::$kind(value) => Ok(value),
                _ => Err(ReflectKindError {
                    expected: ReflectKind::$kind,
                    received: self.kind(),
                }),
            }
        }
    };
}

impl<'a> ReflectRef<'a> {
    impl_kind_fn!();
    impl_cast_fn!(as_struct: Struct => &'a dyn Struct);
    impl_cast_fn!(as_tuple_struct: TupleStruct => &'a dyn TupleStruct);
    impl_cast_fn!(as_tuple: Tuple => &'a dyn Tuple);
    impl_cast_fn!(as_list: List => &'a dyn List);
    impl_cast_fn!(as_array: Array => &'a dyn Array);
    impl_cast_fn!(as_map: Map => &'a dyn Map);
    impl_cast_fn!(as_set: Set => &'a dyn Set);
    impl_cast_fn!(as_enum: Enum => &'a dyn Enum);
    impl_cast_fn!(as_opaque: Opaque => &'a dyn Reflect);
}

impl<'a> ReflectMut<'a> {
    impl_kind_fn!();
    impl_cast_fn!(as_struct: Struct => &'a mut dyn Struct);
    impl_cast_fn!(as_tuple_struct: TupleStruct => &'a mut dyn TupleStruct);
    impl_cast_fn!(as_tuple: Tuple => &'a mut dyn Tuple);
    impl_cast_fn!(as_list: List => &'a mut dyn List);
    impl_cast_fn!(as_array: Array => &'a mut dyn Array);
    impl_cast_fn!(as_map: Map => &'a mut dyn Map);
    impl_cast_fn!(as_set: Set => &'a mut dyn Set);
    impl_cast_fn!(as_enum: Enum => &'a mut dyn Enum);
    impl_cast_fn!(as_opaque: Opaque => &'a mut dyn Reflect);
}

impl ReflectOwned {
    impl_kind_fn!();
    impl_cast_fn!(into_struct: Struct => Box<dyn Struct>);
    impl_cast_fn!(into_tuple_struct: TupleStruct => Box<dyn TupleStruct>);
    impl_cast_fn!(into_tuple: Tuple => Box<dyn Tuple>);
    impl_cast_fn!(into_list: List => Box<dyn List>);
    impl_cast_fn!(into_array: Array => Box<dyn Array>);
    impl_cast_fn!(into_map: Map => Box<dyn Map>);
    impl_cast_fn!(into_set: Set => Box<dyn Set>);
    impl_cast_fn!(into_enum: Enum => Box<dyn Enum>);
    impl_cast_fn!(into_opaque: Opaque => Box<dyn Reflect>);
}
