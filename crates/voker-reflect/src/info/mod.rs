//! Provides compile-time type information and metadata APIs.
//!
//! ## Menu
//!
//! - [`TypeId`]: `core::any::TypeId`, a 16-byte value that uniquely identifies a type at runtime.
//!
//! - [`TypePath`]: A trait for obtaining canonical type names without a leading `::`.
//!     - [`type_path`](TypePath::type_path): Full name, a fixed and unique identifier for the type.
//!     - [`type_name`](TypePath::type_name): The name without module path, may be duplicated.
//!     - [`type_ident`](TypePath::type_ident): The name without generics and module path.
//!     - [`module_path`](TypePath::module_path): Optional module path, for example `voker_reflect::info`.
//!
//! - [`DynamicTypePath`]: Dynamic dispatch support for `TypePath`.
//!
//! - [`TypePathTable`]: A struct storing four function pointers for a single type's `TypePath` implementation.
//!
//! - [`Type`]: A compact type descriptor containing a `TypeId` and a `TypePathTable`.
//!
//! - [`CustomAttributes`]: An attribute container similar to `Map<TypeId, Box<dyn Any>>`.
//!
//! - [`Generics`]: A list of `GenericInfo` values describing instantiated generic parameters.
//!     - [`GenericInfo`]: An enum over `TypeParamInfo` and `ConstParamInfo`.
//!     - [`TypeParamInfo`]: Type-parameter metadata, including parameter name, `Type`, and optional default `Type`.
//!     - [`ConstParamInfo`]: Const-parameter metadata, including parameter name, `Type`, and const value.
//!
//! - [`TypeInfo`]: An enum representing compile-time type metadata. Variants include:
//!     - Note: Each of the following types contains the type's own `Type` and generic metadata.
//!     - [`ArrayInfo`]: Array metadata, such as `[i32; 5]`, including capacity and item type information.
//!     - [`ListInfo`]: List-like metadata, such as `Vec<i32>`, including item type information.
//!     - [`TupleInfo`]: Tuple metadata, such as `(i32, f32)`, including per-field type information.
//!     - [`StructInfo`]: Struct metadata, such as `A { .. }`, including field names, field types, and custom attributes.
//!     - [`TupleStructInfo`]: Tuple-struct metadata, such as `A(..)`, including field types and custom attributes.
//!     - [`EnumInfo`]: Enum metadata, including variant metadata and custom attributes.
//!     - [`MapInfo`]: Map-like metadata, such as `HashMap<K, V>`, including key and value type information.
//!     - [`SetInfo`]: Set-like metadata, such as `HashSet<T>`, including value type information.
//!     - [`OpaqueInfo`]: Metadata for opaque types, such as `struct A;` or `String`.
//!
//! - [`VariantInfo`]: An enum representing enum variant metadata. Variants include:
//!     - Note: Each of the following types contains the variant name and custom attributes.
//!     - [`StructVariantInfo`]: Similar to `StructInfo`, but without generic metadata.
//!     - [`TupleVariantInfo`]: Similar to `TupleInfo`.
//!     - [`UnitVariantInfo`]: No more content.
//!
//! - Field Info:
//!     - [`NamedField`]: Metadata for struct fields, including name, field type, and custom attributes.
//!     - [`UnnamedField`]: Metadata for tuple and tuple-struct fields, including index, field type, and custom attributes.
//!
//! - Kind:
//!     - [`ReflectKind`]: The broad reflection kind, such as `Struct`, `Array`, or `Opaque`.
//!     - [`VariantKind`]: The enum variant kind: `Struct`, `Tuple`, or `Unit`.
//!
//! - [`Typed`]: A trait for obtaining `TypeInfo` for a concrete type.
//!
//! - [`DynamicTyped`]: Dynamic dispatch support for `Typed`.
//!
//! [`TypeId`]: core::any::TypeId

// -----------------------------------------------------------------------------
// Modules

mod array_info;
mod attributes;
mod const_param_data;
mod docs_macro;
mod enum_info;
mod field_info;
mod generics;
mod list_info;
mod map_info;
mod opaque_info;
mod set_info;
mod struct_info;
mod tuple_info;
mod tuple_struct_info;
mod type_info;
mod type_path;
mod typed;
mod variant_info;

// -----------------------------------------------------------------------------
// Internal API

use attributes::{impl_custom_attributes_fn, impl_with_custom_attributes};
use docs_macro::impl_docs_fn;
use generics::impl_generic_fn;

pub(crate) use type_path::impl_type_fn;

// -----------------------------------------------------------------------------
// Exports

pub use voker_reflect_derive::TypePath;

pub use array_info::ArrayInfo;
pub use attributes::CustomAttributes;
pub use const_param_data::ConstParamData;
pub use enum_info::EnumInfo;
pub use field_info::{NamedField, UnnamedField};
pub use generics::{ConstParamInfo, GenericInfo, Generics, TypeParamInfo};
pub use list_info::ListInfo;
pub use map_info::MapInfo;
pub use opaque_info::OpaqueInfo;
pub use set_info::SetInfo;
pub use struct_info::StructInfo;
pub use tuple_info::TupleInfo;
pub use tuple_struct_info::TupleStructInfo;
pub use type_info::{ReflectKind, ReflectKindError, TypeInfo};
pub use type_path::{DynamicTypePath, Type, TypePath, TypePathTable};
pub use typed::{DynamicTyped, Typed};
pub use variant_info::{StructVariantInfo, TupleVariantInfo, UnitVariantInfo};
pub use variant_info::{VariantInfo, VariantKind, VariantKindError};
