//! Provides a type registry for querying reflected metadata without holding values.
//!
//! ## Menu
//!
//! - [`TypeData`]: A trait representing a capability supported by a type.
//! - [`FromType`]: A trait that constructs a `TypeData` from a concrete type.
//! - [`TypeMeta`]: A container including a [`TypeInfo`] and a [`TypeData`] table.
//! - [`GetTypeMeta`]: A trait that constructs a [`TypeMeta`] from a type.
//! - [`TypeRegistry`]: A container for storing and querying [`TypeMeta`] values.
//! - TypeDatas:
//!     - [`ReflectDefault`]: Provides [`Default`] support for reflected types.
//!     - [`ReflectFromPtr`]: Converts raw pointers into reflection references.
//!     - [`ReflectFromReflect`]: Provide [`FromReflect`] support for deserialization.
//!     - [`ReflectSerialize`]: Provides serialization support for reflected types.
//!     - [`ReflectDeserialize`]: Provides deserialization support for reflected types.
//! - [`reflect_trait`]: An attribute macro that generates a `{Trait}FromReflect` helper usable as a [`TypeData`].
//!
//! ## auto_register
//!
//! See [`TypeRegistry::auto_register`].
//!
//! This module uses the [`voker_inventory`] crate for static registration.
//! Not all platforms support it, although major targets do.
//!
//! On unsupported platforms, auto-registration simply returns `false` without failing.
//! Generic types are not collected automatically; register concrete generic
//! instantiations manually when needed.
//!
//! ### auto_register type menu
//!
//! - `()` `bool` `char` `f32` `f64`
//! - `i8` `i16` `i32` `i64` `i128` `isize`
//! - `u8` `u16` `u32` `u64` `u128` `usize`
//! - `core::num::NonZero`: I8-I128 U8-U128 Isize Usize
//! - `Atomic`: Ordering, Bool I8-I64 U8-U64 Isize Usize (without Ptr)
//! - `String` `&'static str` `Cow<'static, str>`
//! - `core::any::TypeId`
//! - `core::time::Duration`
//! - `&'static core::panic::Location<'static>`
//! - `voker_os::time::Instant`
//! - "std" feature:
//!   `OsString` `PathBuf` `Cow<'static, Path>` `&'static Path`
//! - ......
//!
//! [`reflect_trait`]: crate::derive::reflect_trait
//! [`FromReflect`]: crate::FromReflect
//! [`TypeInfo`]: crate::info::TypeInfo

// -----------------------------------------------------------------------------
// Modules

mod auto_register;
mod from_type;
mod traits;
mod type_data;
mod type_meta;
mod type_registry;

// -----------------------------------------------------------------------------
// Exports

pub use auto_register::{AutoRegister, RegisterFn};
pub use from_type::FromType;
pub use traits::ReflectDefault;
pub use traits::{ReflectDeserialize, ReflectSerialize};
pub use traits::{ReflectFromPtr, ReflectFromReflect};
pub use type_data::TypeData;
pub use type_meta::{GetTypeMeta, TypeMeta};
pub use type_registry::{TypeRegistry, TypeRegistryArc};
