use alloc::boxed::Box;

use serde_core::{Deserialize, Deserializer};

use crate::Reflect;
use crate::info::{TypePath, Typed};
use crate::registry::FromType;

/// A container providing `serde` deserialization support for reflected types.
///
/// Internally stores function pointers corresponding to specific types. During deserialization,
/// it invokes the concrete type's `serde` implementation and returns the result as `Box<dyn Reflect>`.
///
/// This is typically used by the internal implementation of [`voker_reflect::serde`].
/// See [`ReflectDeserializeDriver`](voker_reflect::serde::ReflectDeserializeDriver) for details.
///
/// # Examples
///
/// ```
/// use core::any::TypeId;
/// use voker_reflect::registry::{ReflectDeserialize, TypeRegistry};
/// use voker_reflect::Reflect;
/// use serde::Deserialize;
///
/// #[derive(Reflect, Deserialize, PartialEq, Debug)]
/// #[reflect(Deserialize)]
/// struct MyStruct {
///   value: i32
/// }
///
/// let input = r#"(value:123)"#;
///
/// let mut registry = TypeRegistry::new();
/// registry.register::<MyStruct>();
///
/// let processor = registry.get_type_data::<ReflectDeserialize>(TypeId::of::<MyStruct>()).unwrap();
///
/// let mut deserializer = ron::Deserializer::from_str(input).unwrap();
///
/// let val = processor.deserialize(&mut deserializer).unwrap();
///
/// assert_eq!(val.take::<MyStruct>().unwrap(), MyStruct{ value: 123 });
/// ```
#[derive(Clone)]
pub struct ReflectDeserialize {
    func: fn(
        deserializer: &mut dyn erased_serde::Deserializer,
    ) -> Result<Box<dyn Reflect>, erased_serde::Error>,
}

impl ReflectDeserialize {
    /// Deserializes a reflected value.
    ///
    /// See [`ReflectDeserialize`] for examples.
    #[inline(always)]
    pub fn deserialize<'de, D: Deserializer<'de>>(
        &self,
        deserializer: D,
    ) -> Result<Box<dyn Reflect>, D::Error> {
        let mut erased = <dyn erased_serde::Deserializer>::erase(deserializer);
        (self.func)(&mut erased).map_err(<D::Error as serde_core::de::Error>::custom)
    }
}

impl<T: for<'a> Deserialize<'a> + Typed + Reflect> FromType<T> for ReflectDeserialize {
    fn from_type() -> Self {
        Self {
            func: |deserializer| Ok(Box::new(T::deserialize(deserializer)?)),
        }
    }
}

// Explicitly implemented here so that code readers do not need
// to ponder the principles of proc-macros in advance.
impl TypePath for ReflectDeserialize {
    #[inline(always)]
    fn type_path() -> &'static str {
        "voker_reflect::registry::ReflectDeserialize"
    }

    #[inline(always)]
    fn type_name() -> &'static str {
        "ReflectDeserialize"
    }

    #[inline(always)]
    fn type_ident() -> &'static str {
        "ReflectDeserialize"
    }

    #[inline(always)]
    fn module_path() -> Option<&'static str> {
        Some("voker_reflect::registry")
    }
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use super::ReflectDeserialize;
    use crate::info::TypePath;

    #[test]
    fn type_path() {
        assert!(ReflectDeserialize::type_path() == "voker_reflect::registry::ReflectDeserialize");
        assert!(ReflectDeserialize::module_path() == Some("voker_reflect::registry"));
        assert!(ReflectDeserialize::type_ident() == "ReflectDeserialize");
        assert!(ReflectDeserialize::type_name() == "ReflectDeserialize");
    }
}
