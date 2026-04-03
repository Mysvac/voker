use alloc::boxed::Box;

use crate::Reflect;
use crate::info::{TypePath, Typed};
use crate::registry::FromType;

/// A container providing [`Default`] support for reflected types.
///
/// Use this to create a reflected default value via [`TypeRegistry`] and [`TypeId`] (or [`TypePath`]).
///
/// # Creating a instance
///
/// You can create one directly using [`FromType`]:
///
/// ```
/// use voker_reflect::prelude::*;
/// use voker_reflect::registry::FromType;
///
/// #[derive(Reflect, Default)]
/// struct Foo;
///
/// let defaulter: ReflectDefault = FromType::<Foo>::from_type();
/// ```
///
/// # Automatic registration
///
/// After calling [`TypeRegistry::auto_register`], [`ReflectDefault`] is available
/// for many commonly used types:
///
/// - Integer types: `u8`-`u128`, `i8`-`i128`, `usize`, `isize`
/// - Primitives: `()`, `bool`, `char`, `f32`, `f64`
/// - String types: `String`, `&'static str`
/// - Collections: `Vec<T>`, `BinaryHeap<T>`, `VecDeque<T>`
/// - Map types: `BTreeMap<K, V>`, `BTreeSet<T>`
/// - Others: `Option<T>`, `PhantomData<T>`, `Duration` ...
///
/// [`TypeRegistry::auto_register`]: crate::registry::TypeRegistry::auto_register
///
/// ```
/// use voker_reflect::prelude::*;
///
/// let mut registry = TypeRegistry::new();
/// registry.auto_register();
///
/// let generator = registry
///     .get_by_name("String").unwrap()
///     .get_data::<ReflectDefault>().unwrap();
///
/// let s: Box<dyn Reflect> = generator.default();
///
/// assert_eq!(s.take::<String>().unwrap(), "");
/// ```
///
/// # Derive macro support
///
/// If a type implements `Default` and is annotated with `#[reflect(default)]`, [`ReflectDefault`]
/// will be automatically registered when the type is added to the registry:
///
/// ```
/// use core::any::TypeId;
/// use voker_reflect::prelude::*;
///
/// #[derive(Reflect, Default)]
/// #[reflect(default)]
/// struct Foo;
///
/// let mut registry = TypeRegistry::default();
/// registry.register::<Foo>();
///
/// let defaulter = registry.get_type_data::<ReflectDefault>(TypeId::of::<Foo>());
/// assert!(defaulter.is_some());
/// ```
///
/// # Manual registration
///
/// If you're unsure whether a type has [`ReflectDefault`] registered, you can add it manually:
///
/// ```
/// use core::any::TypeId;
/// use voker_reflect::prelude::*;
///
/// #[derive(Reflect, Default)]
/// struct Foo;
///
/// let mut registry = TypeRegistry::default();
/// registry.register::<Foo>();
/// registry.register_type_data::<Foo, ReflectDefault>();
///
/// let defaulter = registry.get_type_data::<ReflectDefault>(TypeId::of::<Foo>());
/// assert!(defaulter.is_some());
/// ```
///
/// [`TypePath`]: crate::info::TypePath::type_path
/// [`TypeRegistry`]: crate::registry::TypeRegistry
/// [`TypeId`]: core::any::TypeId
#[derive(Clone)]
pub struct ReflectDefault {
    func: fn() -> Box<dyn Reflect>,
}

impl ReflectDefault {
    /// Call T's [`Default`]
    ///
    /// [`ReflectDefault`] does not have a type flag,
    /// but the functions used internally are type specific.
    #[inline(always)]
    pub fn default(&self) -> Box<dyn Reflect> {
        (self.func)()
    }
}

impl<T: Default + Typed + Reflect> FromType<T> for ReflectDefault {
    fn from_type() -> Self {
        Self {
            func: || Box::<T>::default(),
        }
    }
}

// Explicitly implemented here so that code readers do not need
// to ponder the principles of proc-macros in advance.
impl TypePath for ReflectDefault {
    #[inline(always)]
    fn type_path() -> &'static str {
        "voker_reflect::registry::ReflectDefault"
    }

    #[inline(always)]
    fn type_name() -> &'static str {
        "ReflectDefault"
    }

    #[inline(always)]
    fn type_ident() -> &'static str {
        "ReflectDefault"
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
    use super::ReflectDefault;
    use crate::info::TypePath;

    #[test]
    fn type_path() {
        assert!(ReflectDefault::type_path() == "voker_reflect::registry::ReflectDefault");
        assert!(ReflectDefault::module_path() == Some("voker_reflect::registry"));
        assert!(ReflectDefault::type_ident() == "ReflectDefault");
        assert!(ReflectDefault::type_name() == "ReflectDefault");
    }
}
