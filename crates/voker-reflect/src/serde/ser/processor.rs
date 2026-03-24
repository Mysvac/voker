use serde_core::Serializer;

use crate::Reflect;
use crate::registry::TypeRegistry;

/// A trait for types that support dynamic serialization through reflection.
///
/// Implementors serialize data by combining runtime type information
/// ([`&dyn Reflect`]), the type registry ([`&TypeRegistry`]), and a [`serde::Serializer`].
///
/// ## Return Value Semantics
///
/// The trait returns `Result<Result<T, S::Error>, S>` with three distinct outcomes:
///
/// - **`Ok(Ok(serialized))`** → Successful serialization
/// - **`Ok(Err(error))`** → Type is supported but serialization failed (e.g., invalid data)
/// - **`Err(serializer)`** → Type is not supported; serializer is returned for alternative strategies
///
/// ## Default Implementation
///
/// The trait is implemented for `()` as a default processor that always returns `Err(serializer)`
/// (indicating no support for any type).
///
/// This does not mean that serialization is not supported, [`SerializeDriver`] will still
/// try using [`ReflectSerialize`] and default serialization methods.
///
/// See serialization rules in [`SerializeDriver`] .
///
/// [`ReflectSerialize`]: crate::registry::ReflectSerialize
/// [`SerializeDriver`]: crate::serde::SerializeDriver
/// [`&dyn Reflect`]: crate::Reflect
/// [`&TypeRegistry`]: crate::registry::TypeRegistry
/// [`serde::Serializer`]: serde_core::Serializer
pub trait SerializeProcessor {
    fn try_serialize<S: Serializer>(
        &self,
        value: &dyn Reflect,
        registry: &TypeRegistry,
        serializer: S,
    ) -> Result<Result<S::Ok, S::Error>, S>;
}

impl SerializeProcessor for () {
    #[inline]
    fn try_serialize<S: Serializer>(
        &self,
        _value: &dyn Reflect,
        _registry: &TypeRegistry,
        serializer: S,
    ) -> Result<Result<S::Ok, S::Error>, S> {
        Err(serializer)
    }
}
