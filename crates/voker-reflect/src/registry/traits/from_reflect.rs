use alloc::boxed::Box;

use crate::info::{TypePath, Typed};
use crate::registry::FromType;
use crate::{FromReflect, Reflect};

/// A function pointer container that enables dynamic conversion from reflected types.
///
/// While [`FromReflect`] allows conversion when the target type is statically known,
/// this container enables dynamic lookup and invocation using only type identifiers.
///
/// Primarily used in reflection-based deserialization where the target type must be
/// determined at runtime based on type paths or identifiers retrieved from serialized data.
///
/// # Examples
///
/// ```
/// # use voker_reflect::{Reflect, registry::{TypeRegistry, ReflectFromReflect}};
/// let s: Box<dyn Reflect> = Box::new("123".to_string());
///
/// let registry = TypeRegistry::new(); // `new` will register some basic type
/// let meta = registry.get_with_type_name("String").unwrap();
/// let from_reflect = meta.get_trait::<ReflectFromReflect>().unwrap();
///
/// let s2 = from_reflect.from_reflect(&*s).unwrap();
/// assert_eq!(s2.take::<String>().unwrap(), "123");
/// ```
#[derive(Clone)]
pub struct ReflectFromReflect {
    func: fn(&dyn Reflect) -> Option<Box<dyn Reflect>>,
}

impl ReflectFromReflect {
    /// Call T's [`Reflect`]
    ///
    /// [`ReflectFromReflect`] does not have a type flag,
    /// but the functions used internally are type specific.
    #[inline(always)]
    pub fn from_reflect(&self, param_1: &dyn Reflect) -> Option<Box<dyn Reflect>> {
        (self.func)(param_1)
    }
}

impl<T: Typed + FromReflect> FromType<T> for ReflectFromReflect {
    fn from_type() -> Self {
        Self {
            func: |param_1| T::from_reflect(param_1).map(Reflect::into_boxed_reflect),
        }
    }
}

// Explicitly implemented here so that code readers do not need
// to ponder the principles of proc-macros in advance.
impl TypePath for ReflectFromReflect {
    #[inline(always)]
    fn type_path() -> &'static str {
        "voker_reflect::registry::ReflectFromReflect"
    }

    #[inline(always)]
    fn type_name() -> &'static str {
        "ReflectFromReflect"
    }

    #[inline(always)]
    fn type_ident() -> &'static str {
        "ReflectFromReflect"
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
    use super::ReflectFromReflect;
    use crate::info::TypePath;

    #[test]
    fn type_path() {
        assert!(ReflectFromReflect::type_path() == "voker_reflect::registry::ReflectFromReflect");
        assert!(ReflectFromReflect::module_path() == Some("voker_reflect::registry"));
        assert!(ReflectFromReflect::type_ident() == "ReflectFromReflect");
        assert!(ReflectFromReflect::type_name() == "ReflectFromReflect");
    }
}
