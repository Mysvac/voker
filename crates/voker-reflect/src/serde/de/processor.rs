use alloc::boxed::Box;

use serde_core::Deserializer;

use crate::Reflect;
use crate::registry::{TypeMeta, TypeRegistry};

/// A trait for types that support dynamic deserialization through reflection.
///
/// Implementors deserialize data by combining runtime type information ([`&dyn Reflect`]),
/// the type registry ([`&TypeRegistry`]), type metadata ([`&TypeMeta`]), and a [`serde::Deserializer`].
///
/// ## Return Value Semantics
///
/// The trait returns `Result<Result<T, D::Error>, D>` with three distinct outcomes:
///
/// - **`Ok(Ok(deserialized))`** → Successful deserialization
/// - **`Ok(Err(error))`** → Type is supported but deserialization failed (e.g., invalid data)
/// - **`Err(deserializer)`** → Type is not supported; deserializer is returned for alternative strategies
///
/// ## Default Implementation
///
/// The trait is implemented for `()` as a default processor that always returns `Err(deserializer)`
/// (indicating no support for any type).
///
/// This does not mean that deserialization is not supported, [`DeserializeDriver`] will still
/// try using [`ReflectDeserialize`] and default deserialization methods.
///
/// See deserialization rules in [`DeserializeDriver`] .
///
/// [`ReflectDeserialize`]: crate::registry::ReflectDeserialize
/// [`DeserializeDriver`]: crate::serde::DeserializeDriver
/// [`&dyn Reflect`]: crate::Reflect
/// [`&TypeRegistry`]: crate::registry::TypeRegistry
/// [`&TypeMeta`]: crate::registry::TypeMeta
/// [`serde::Deserializer`]: serde_core::Deserializer
pub trait DeserializeProcessor {
    fn try_deserialize<'de, D: Deserializer<'de>>(
        &mut self,
        registration: &TypeMeta,
        registry: &TypeRegistry,
        deserializer: D,
    ) -> Result<Result<Box<dyn Reflect>, D::Error>, D>;
}

impl DeserializeProcessor for () {
    fn try_deserialize<'de, D: Deserializer<'de>>(
        &mut self,
        _registration: &TypeMeta,
        _registry: &TypeRegistry,
        deserializer: D,
    ) -> Result<Result<Box<dyn Reflect>, D::Error>, D> {
        Err(deserializer)
    }
}
