//! Provide serialization and deserialization support for the reflection system.
//!
//! This module primarily serves two purposes:
//!
//! 1. Enables serialization and deserialization under type-erased conditions
//! 2. Allows composite types to be serialized and deserialized based on the reflection system's
//!    type information, even without implementing `serde` traits
//!
//! # Overview
//!
//! ## Serialization
//!
//! - [`ReflectSerialize`]: Stores function pointers that enable dynamic types to invoke
//!   `serde`'s serialization implementations.
//! - [`SerializeProcessor`]: Serialization processor that allows users to customize
//!   serialization behavior.
//! - [`SerializeDriver`]: Standard serializer that follows a priority-based dispatch strategy.
//!     - First attempts to use [`SerializeProcessor`]; if supported, returns its result immediately.
//!     - Then queries and invokes [`ReflectSerialize`] if available.
//!     - Finally falls back to reflection-based serialization (unavailable for Opaque types).
//! - [`ReflectSerializeDriver`]: Wraps serialized data with type path mapping to preserve type information.
//!     - Only the outermost layer includes type paths; inner data types are inferred from field names,
//!       using [`SerializeDriver`] internally.
//!
//! ### Examples
//!
//! ```no_run
//! # use voker_reflect::prelude::{TypeRegistry, ReflectSerializeDriver, Reflect};
//! #
//! #[derive(Reflect)]
//! #[reflect(type_path = "my_crate::MyStruct")]
//! struct MyStruct {
//!   value: i32
//! }
//!
//! let mut registry = TypeRegistry::new();
//! registry.register::<MyStruct>();
//!
//! let input = MyStruct { value: 123 };
//!
//! let driver = ReflectSerializeDriver::new(&input, &registry);
//! let output = ron::to_string(&driver).unwrap();
//!
//! assert_eq!(output, r#"{"my_crate::MyStruct":(value:123)}"#);
//! ```
//!
//! ## Deserialization
//!
//! - [`ReflectDeserialize`]: Stores function pointers that enable dynamic types to invoke
//!   `serde`'s deserialization implementations.
//! - [`DeserializeProcessor`]: Deserialization processor that allows users to customize
//!   deserialization behavior.
//! - [`DeserializeDriver`]: Standard deserializer that follows a priority-based dispatch strategy.
//!     - First attempts to use [`DeserializeProcessor`]; if supported, returns its result immediately.
//!     - Then queries and invokes [`ReflectDeserialize`] if available.
//!     - Finally falls back to reflection-based deserialization (unavailable for Opaque types).
//!     - Corresponds to [`SerializeDriver`]; requires explicit [`TypeMeta`] since data lacks type information.
//! - [`ReflectDeserializeDriver`]: Parses data with type path mapping (corresponds to [`ReflectSerializeDriver`]).
//!     - Type paths embedded in the data allow lookup of [`TypeMeta`] from the registry,
//!       eliminating the need for manual specification.
//!     - Only the outermost layer requires type paths; inner data types are inferred from field names,
//!       using [`DeserializeDriver`] internally.
//!
//! ### Examples
//!
//! ```no_run
//! # use serde_core::de::DeserializeSeed;
//! # use voker_reflect::ops::DynamicStruct;
//! # use voker_reflect::prelude::{Reflect, FromReflect, TypeRegistry, ReflectDeserializeDriver};
//! #
//! #[derive(Reflect, PartialEq, Debug)]
//! #[reflect(type_path = "my_crate::MyStruct")]
//! struct MyStruct {
//!   value: i32
//! }
//!
//! let mut registry = TypeRegistry::new();
//! registry.register::<MyStruct>();
//!
//! let input = r#"{
//!   "my_crate::MyStruct": (
//!     value: 123
//!   )
//! }"#;
//!
//! let mut data = ron::Deserializer::from_str(input).unwrap();
//! let driver = ReflectDeserializeDriver::new(&registry);
//!
//! let output: Box<dyn Reflect> = driver.deserialize(&mut data).unwrap();
//!
//! // If `ReflectFromReflect` is registered for this represented type,
//! // the driver performs one final conversion attempt from the dynamic value.
//! // If it is not registered (or conversion fails), the dynamic value is
//! // returned unchanged.
//! assert!(output.is::<MyStruct>());
//!
//! assert_eq!(output.take::<MyStruct>().unwrap(), MyStruct { value: 123 });
//! ```
//!
//! ## Field Skipping
//!
//! A special attribute `skip_serde` enables skipping fields during both serialization and deserialization.
//!  
//! This attribute **only** works with the default implementation (reflection-based serialization/deserialization).
//! It has **no effect** when custom processors are provided or when types have native `serde` implementations.
//!
//! ### Examples
//!
//! ```no_run
//! # use core::marker::PhantomData;
//! # use voker_reflect::Reflect;
//! #[derive(Reflect)]
//! struct Foo<T> {
//!     data: u64,
//!     #[reflect(skip_serde)]
//!     _marker: PhantomData<T>,
//! }
//! ```
//!
//! [`TypeMeta`]: crate::registry::TypeMeta
//! [`ReflectDeserialize`]: crate::registry::ReflectDeserialize
//! [`ReflectSerialize`]: crate::registry::ReflectSerialize

// -----------------------------------------------------------------------------
// Debug utils

crate::cfg::backtrace! {
    mod info_stack;
    use info_stack::TypeInfoStack;
}

// -----------------------------------------------------------------------------
// Modules

mod de;
mod ser;

// -----------------------------------------------------------------------------
// Exports

pub use de::{DeserializeDriver, DeserializeProcessor, ReflectDeserializeDriver};
pub use ser::{ReflectSerializeDriver, SerializeDriver, SerializeProcessor};
