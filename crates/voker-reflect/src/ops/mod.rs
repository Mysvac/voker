//! Provide interfaces and dynamic types for data operation.
//!
//! ## Menu
//!
//! ### Interface
//!
//! The following is the subtrait of [`Reflect`], which provide data access methods in different types.
//!
//! - [`Array`]: For array (e.g. `[i32; 5]`) .
//! - [`Tuple`]: For tuple (e.g. `(i32, f32)`) .
//! - [`List`]: For list-like (e.g. `Vec<i32>`) .
//! - [`Struct`]: For struct (e.g. `A{ .. }`) .
//! - [`TupleStruct`]: For tuple-struct (e.g. `A(..)`) .
//! - [`Map`]: For map-like (e.g. `HashMap<i32, f32>`) .
//! - [`Set`]: For set-like (e.g. `HashSet<i32>`) .
//! - [`Enum`]: For **a variant of enum**, e.g. `Option::Some(T)`.
//!
//! ### Dynamic Type
//!
//! The dynamic types usually are used in serialization and deserialization,
//! supported to dynamic adding or removing fields.
//!
//! - [`DynamicArray`]: representing array data, with fixed length, similar to `Box<[Box<dyn Reflect>]>`.
//! - [`DynamicList`]: representing list-like data, similar to `Vec<Box<dyn Reflect>>`.
//! - [`DynamicTuple`]: representing tuple data, similar to `Vec<Box<dyn Reflect>>`.
//! - [`DynamicStruct`]: representing struct data, similar to `Map<String, Box<dyn Reflect>>`.
//! - [`DynamicTupleStruct`]: representing tuple-struct data, similar to `Vec<Box<dyn Reflect>>`.
//! - [`DynamicMap`]: representing map-like data, similar to `Map<Box<dyn Reflect>, Box<dyn Reflect>>`.
//! - [`DynamicSet`]: representing set-like data, similar to `Set<Box<dyn Reflect>>`.
//! - [`DynamicEnum`]: representing **a variant of enum**, it's `DynamicVariant` + `EnumInfo` + `VariantName`.
//! - [`DynamicVariant`]: A enum representing enum variant, one of `()`, `DynamicStruct`, `DynamicTuple`.
//!
//! For these, we have placed all data access methods in the subtrait of [`Reflect`] (e.g. [`Array`], [`Struct`]).
//!
//! Dynamic types are special in that their `TypeInfo` is [`OpaqueInfo`],
//! but other APIs behave like the represented type, such as [`reflect_kind`] and [`reflect_ref`].
//!
//! [`Reflect`]: crate::Reflect
//! [`Array`]: crate::ops::Array
//! [`Struct`]: crate::ops::Struct
//! [`OpaqueInfo`]: crate::info::OpaqueInfo
//! [`reflect_kind`]: crate::Reflect::reflect_kind
//! [`reflect_ref`]: crate::Reflect::reflect_ref

// -----------------------------------------------------------------------------
// Modules

mod apply_error;
mod array_ops;
mod clone_error;
mod enum_ops;
mod kind;
mod list_ops;
mod map_ops;
mod set_ops;
mod struct_ops;
mod tuple_ops;
mod tuple_struct_ops;
mod variant_ops;

// -----------------------------------------------------------------------------
// Exports

pub use apply_error::ApplyError;
pub use clone_error::ReflectCloneError;

pub use kind::{ReflectMut, ReflectOwned, ReflectRef};

pub use array_ops::{Array, ArrayItemIter, DynamicArray};
pub use enum_ops::{DynamicEnum, Enum};
pub use list_ops::{DynamicList, List, ListItemIter};
pub use map_ops::{DynamicMap, Map};
pub use set_ops::{DynamicSet, Set};
pub use struct_ops::{DynamicStruct, Struct, StructFieldIter};
pub use tuple_ops::{DynamicTuple, Tuple, TupleFieldIter};
pub use tuple_struct_ops::{DynamicTupleStruct, TupleStruct, TupleStructFieldIter};
pub use variant_ops::{DynamicVariant, VariantField, VariantFieldIter};
