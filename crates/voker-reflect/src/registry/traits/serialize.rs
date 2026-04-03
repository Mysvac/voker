use serde_core::{Serialize, Serializer};

use crate::Reflect;
use crate::info::{TypePath, Typed};
use crate::registry::FromType;

/// A container providing `serde` serialization support for reflected types.
///
/// Internally stores function pointers corresponding to specific types. When given a reflected type,
/// it downcasts to the concrete type and invokes the `serde` serialization functions.
///
/// This is typically used by the internal implementation of [`voker_reflect::serde`].
/// See [`ReflectSerializeDriver`](voker_reflect::serde::ReflectSerializeDriver) for details.
///
/// # Safety
///
/// Passing an incorrectly typed `&dyn Reflect` value will cause a panic.
///
/// # Examples
///
/// ```
/// use core::any::TypeId;
/// use voker_reflect::registry::{ReflectSerialize, TypeRegistry};
/// use voker_reflect::Reflect;
/// use serde::Serialize;
///
/// #[derive(Reflect, Serialize, PartialEq, Debug)]
/// #[reflect(serialize)]
/// struct MyStruct {
///   value: i32
/// }
///
/// let mut registry = TypeRegistry::new();
/// registry.register::<MyStruct>();
///
/// let input = MyStruct { value: 123 };
///
/// let processor = registry.get_type_data::<ReflectSerialize>(TypeId::of::<MyStruct>()).unwrap();
///
/// let mut output = String::new();
/// let mut serializer = ron::Serializer::new(&mut output, None).unwrap();
///
/// processor.serialize(&input, &mut serializer);
///
/// assert_eq!(output, r#"(value:123)"#);
/// ```
#[derive(Clone)]
pub struct ReflectSerialize {
    fun: fn(value: &dyn Reflect) -> &dyn erased_serde::Serialize,
}

impl<T: Serialize + Typed + Reflect> FromType<T> for ReflectSerialize {
    fn from_type() -> Self {
        Self {
            fun: |value| match value.downcast_ref::<T>() {
                Some(val) => val as &dyn erased_serde::Serialize,
                None => {
                    panic!(
                        "Serial type mismatched, Serial Type `{}` with Value Type: {}",
                        T::type_path(),
                        value.reflect_type_path(),
                    );
                }
            },
        }
    }
}

impl ReflectSerialize {
    /// Serializes a reflected value.
    ///
    /// See [`ReflectSerialize`] for examples.
    ///
    /// # Panic
    ///
    /// - Mismatched Type
    #[inline(always)]
    pub fn serialize<S: Serializer>(
        &self,
        value: &dyn Reflect,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        (self.fun)(value).serialize(serializer)
    }
}

// Explicitly implemented here so that code readers do not need
// to ponder the principles of proc-macros in advance.
impl TypePath for ReflectSerialize {
    #[inline(always)]
    fn type_path() -> &'static str {
        "voker_reflect::registry::ReflectSerialize"
    }

    #[inline(always)]
    fn type_name() -> &'static str {
        "ReflectSerialize"
    }

    #[inline(always)]
    fn type_ident() -> &'static str {
        "ReflectSerialize"
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
    use super::ReflectSerialize;
    use crate::info::TypePath;

    #[test]
    fn type_path() {
        assert!(ReflectSerialize::type_path() == "voker_reflect::registry::ReflectSerialize");
        assert!(ReflectSerialize::module_path() == Some("voker_reflect::registry"));
        assert!(ReflectSerialize::type_ident() == "ReflectSerialize");
        assert!(ReflectSerialize::type_name() == "ReflectSerialize");
    }
}
