use core::any::TypeId;

use crate::derive::Reflect;
use crate::prelude::TypeMeta;
use crate::registry::{GetTypeMeta, TypeRegistry};
use voker_inventory as inv;

/// Internal type used to implement automatic registration
/// by collecting registration function pointers.
pub struct RegisterFn(fn(&mut TypeRegistry) -> &mut TypeRegistry);

/// A trait for avoiding duplicate registration.
pub trait AutoRegister: GetTypeMeta {}

impl RegisterFn {
    pub const fn of<T: GetTypeMeta>() -> Self {
        Self(TypeRegistry::register::<T>)
    }
}

inv::collect!(RegisterFn);

inv::submit!(RegisterFn::of::<AutoRegisterFlag>() => RegisterFn);

/// A flag used to mark that automatic registration has been completed.
#[derive(Reflect)]
#[reflect(FromReflect = false, GetTypeMeta = false)]
pub struct AutoRegisterFlag;

impl AutoRegister for AutoRegisterFlag {}

impl GetTypeMeta for AutoRegisterFlag {
    fn get_type_meta() -> TypeMeta {
        TypeMeta::new::<Self>()
    }
}

impl TypeRegistry {
    /// Automatically registers all non-generic types derived with
    /// [`#[derive(Reflect)]`](crate::derive::Reflect), or explicitly
    /// declared via [`impl_auto_register!`] / [`auto_register!`].
    ///
    /// [`impl_auto_register!`]: crate::derive::impl_auto_register
    /// [`auto_register!`]: crate::derive::auto_register
    ///
    /// This method is equivalent to calling [`register`](Self::register) for each qualifying type.
    /// Repeated calls are cheap and will not insert duplicates.
    ///
    /// ## Return Value
    ///
    /// Returns `true` if automatic registration is supported and succeeds
    /// on the current platform; otherwise, returns `false`.
    ///
    /// Once registration succeeds, subsequent calls remain cheap, do not
    /// insert duplicates, and continue to return `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::any::TypeId;
    /// # use voker_reflect::{Reflect, registry::{TypeRegistry, ReflectDefault}};
    /// #[derive(Reflect, Default)]
    /// #[reflect(default)]
    /// struct Foo {
    ///     name: Option<String>,
    ///     value: i32,
    /// }
    ///
    /// let mut registry = TypeRegistry::new();
    /// let successful = registry.auto_register();
    /// assert!(successful);
    ///
    /// // Main type is registered
    /// assert!(registry.contains(TypeId::of::<Foo>()));
    ///
    /// // Type dependencies are also registered
    /// assert!(registry.contains(TypeId::of::<Option<String>>()));
    /// assert!(registry.contains(TypeId::of::<i32>()));
    ///
    /// // Associated type data is available
    /// let ctor = registry.get_type_data::<ReflectDefault>(TypeId::of::<Foo>());
    /// assert!(ctor.is_some());
    /// ```
    ///
    /// Generic and non-generic types:
    ///
    /// ```
    /// # use std::any::TypeId;
    /// # use voker_reflect::prelude::{Reflect, TypeRegistry};
    /// # use voker_reflect::derive::impl_auto_register;
    ///
    /// #[derive(Reflect)]
    /// struct Foo;
    ///
    /// #[derive(Reflect)]
    /// struct Bar<T>(T);
    ///
    /// impl_auto_register!(Bar<i8>);
    ///
    /// let mut registry = TypeRegistry::new();
    /// registry.auto_register();
    ///
    /// // Non-generic types are automatically registered
    /// assert!(registry.contains(TypeId::of::<Foo>()));
    ///
    /// // Generic types must be explicitly specified
    /// assert!(!registry.contains(TypeId::of::<Bar<u8>>()));
    /// assert!(registry.contains(TypeId::of::<Bar<i8>>()));
    /// ```
    pub fn auto_register(&mut self) -> bool {
        if self.contains(TypeId::of::<AutoRegisterFlag>()) {
            return true;
        }

        for func in inv::iter::<RegisterFn>() {
            (func.0)(self);
        }

        self.contains(TypeId::of::<AutoRegisterFlag>())
    }
}
