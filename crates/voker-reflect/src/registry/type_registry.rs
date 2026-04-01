use alloc::string::String;
use core::any::TypeId;

use voker_utils::extra::TypeIdMap;
use voker_utils::hash::{HashMap, HashSet};

use crate::info::{TypeInfo, Typed};
use crate::registry::{FromType, GetTypeMeta, TypeMeta, TypeTrait};

// -----------------------------------------------------------------------------
// TypeRegistry

/// A registry of [reflected] types.
///
/// This struct is used as the central store for type information.
/// [Registering] a type will generate a new [`TypeMeta`] entry in this store
/// using a type's [`GetTypeMeta`] implementation
/// (which is automatically implemented when using [`#[derive(Reflect)]`](crate::derive::Reflect)).
///
/// It will be used during deserialization, but can also be used for many interesting things.
///
/// # Example
///
/// ```
/// use voker_reflect::registry::{TypeRegistry, ReflectDefault};
/// use voker_reflect::info::DynamicTypePath;
///
/// let input = "String";
/// let registry = TypeRegistry::new();
///
/// let generator = registry
///     .get_with_type_name(input).unwrap()
///     .get_trait::<ReflectDefault>().unwrap();
///
/// let s = generator.default();
/// assert_eq!(s.reflect_type_path(), "alloc::string::String");
///
/// let s = s.take::<String>().unwrap();
/// assert_eq!(s, "");
/// ```
///
/// [reflected]: crate
/// [Registering]: TypeRegistry::register
/// [crate-level documentation]: crate
pub struct TypeRegistry {
    type_meta_table: TypeIdMap<TypeMeta>,
    type_path_to_id: HashMap<&'static str, TypeId>,
    type_name_to_id: HashMap<&'static str, TypeId>,
    ambiguous_names: HashSet<&'static str>,
}

impl Default for TypeRegistry {
    /// See [`TypeRegistry::new`] .
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl TypeRegistry {
    /// Create a empty [`TypeRegistry`].
    #[inline]
    pub const fn empty() -> Self {
        Self {
            type_meta_table: TypeIdMap::new(),
            type_path_to_id: HashMap::new(),
            type_name_to_id: HashMap::new(),
            ambiguous_names: HashSet::new(),
        }
    }

    /// Create a type registry with default registrations for primitive types.
    ///
    /// - `()` `bool` `char`
    /// - `i8 - i128` `isize`
    /// - `u8 - u128` `usize`
    /// - `f32` `f64`
    /// - `String`
    pub fn new() -> Self {
        let mut registry = Self::empty();
        registry.register::<()>();
        registry.register::<bool>();
        registry.register::<char>();
        registry.register::<u8>();
        registry.register::<u16>();
        registry.register::<u32>();
        registry.register::<u64>();
        registry.register::<u128>();
        registry.register::<usize>();
        registry.register::<i8>();
        registry.register::<i16>();
        registry.register::<i32>();
        registry.register::<i64>();
        registry.register::<i128>();
        registry.register::<isize>();
        registry.register::<f32>();
        registry.register::<f64>();
        registry.register::<String>();
        registry
    }

    // # Validity
    // The type must **not** already exist.
    fn add_new_type_indices(
        type_meta: &TypeMeta,
        type_path_to_id: &mut HashMap<&'static str, TypeId>,
        type_name_to_id: &mut HashMap<&'static str, TypeId>,
        ambiguous_names: &mut HashSet<&'static str>,
    ) {
        let ty = type_meta.ty();
        let type_name = ty.name();

        // Check for duplicate names.
        // The type should **not** already exist.
        if !ambiguous_names.contains(type_name) {
            if type_name_to_id.contains_key(type_name) {
                type_name_to_id.remove(type_name);
                ambiguous_names.insert(type_name);
            } else {
                type_name_to_id.insert(type_name, ty.id());
            }
        }

        // For new type, assuming that the full path cannot be duplicated.
        type_path_to_id.insert(ty.path(), ty.id());
    }

    // - If key [`TypeId`] has already exist, the function will do nothing and return `false`.
    // - If the key [`TypeId`] does not exist, the function will insert value and return `true`.
    fn register_internal(&mut self, type_id: TypeId, get_type_meta: fn() -> TypeMeta) -> bool {
        self.type_meta_table.try_insert(type_id, || {
            let meta = get_type_meta();
            Self::add_new_type_indices(
                &meta,
                &mut self.type_path_to_id,
                &mut self.type_name_to_id,
                &mut self.ambiguous_names,
            );
            meta
        })
    }

    /// Try add or do nothing.
    ///
    /// This function checks whether `TypeMeta.type_id()` already exists.  
    /// - If the key [`TypeId`] already exists, the function does nothing and returns `false`.
    /// - If the key [`TypeId`] does not exist, the function will insert value and return `true`.
    ///
    /// This method will _not_ register type dependencies.
    /// Use [`register`](Self::register) to register a type with its dependencies.
    #[inline(always)]
    pub fn try_insert_type_meta(&mut self, type_meta: TypeMeta) -> bool {
        self.type_meta_table.try_insert(type_meta.type_id(), || {
            Self::add_new_type_indices(
                &type_meta,
                &mut self.type_path_to_id,
                &mut self.type_name_to_id,
                &mut self.ambiguous_names,
            );
            type_meta
        })
    }

    /// Insert or **Overwrite** inner TypeTraits.
    ///
    /// This function checks whether `TypeMeta.type_id()` already exists.  
    /// - If the key [`TypeId`] already exists, the value will be overwritten.
    ///   But full_path and type_name table will not be modified.  
    /// - If the key [`TypeId`] does not exist, the value will be inserted.
    ///   And type path will be inserted to full_path and type_name table.
    ///
    /// This method will _not_ register type dependencies.
    /// Use [`register`](Self::register) to register a type with its dependencies.
    pub fn insert_type_meta(&mut self, type_meta: TypeMeta) {
        if !self.type_meta_table.contains(type_meta.type_id()) {
            Self::add_new_type_indices(
                &type_meta,
                &mut self.type_path_to_id,
                &mut self.type_name_to_id,
                &mut self.ambiguous_names,
            );
        }
        self.type_meta_table.insert(type_meta.type_id(), type_meta);
    }

    /// Attempts to register the type `T` if it has not yet been registered already.
    ///
    /// This will also recursively register any type dependencies as specified by [`GetTypeMeta::register_dependencies`].
    /// When deriving `Reflect`, this will generally be all the fields of the struct or enum variant.
    /// As with any type meta, these type dependencies will not be registered more than once.
    ///
    /// If the meta for type `T` already exists, it will not be registered again and neither will its type dependencies.
    /// To register the type, overwriting any existing meta, use [`insert_type_meta`](Self::insert_type_meta) instead.
    ///
    /// Additionally, this will add any reflect [type trait](TypeTrait) as specified in the `Reflect` derive.
    ///
    /// # Example
    ///
    /// ```
    /// # use core::any::TypeId;
    /// # use voker_reflect::{Reflect, registry::{TypeRegistry, ReflectDefault}};
    /// #[derive(Reflect, Default)]
    /// #[reflect(default)]
    /// struct Foo {
    ///   name: Option<String>,
    ///   value: i32
    /// }
    ///
    /// let mut type_registry = TypeRegistry::default();
    ///
    /// type_registry.register::<Foo>();
    ///
    /// // The main type
    /// assert!(type_registry.contains(TypeId::of::<Foo>()));
    ///
    /// // Its type dependencies
    /// assert!(type_registry.contains(TypeId::of::<Option<String>>()));
    /// assert!(type_registry.contains(TypeId::of::<i32>()));
    ///
    /// // Its type data
    /// assert!(type_registry.get_type_trait::<ReflectDefault>(TypeId::of::<Foo>()).is_some());
    /// ```
    pub fn register<T: GetTypeMeta>(&mut self) -> &mut Self {
        if self.register_internal(TypeId::of::<T>(), T::get_type_meta) {
            T::register_dependencies(self);
        }
        self
    }

    /// Attempts to register the referenced type `T` if it has not yet been registered.
    ///
    /// See [`register`](TypeRegistry::register) for more details.
    #[inline]
    pub fn register_by_val<T: GetTypeMeta>(&mut self, _: &T) -> &mut Self {
        self.register::<T>()
    }

    /// Registers the type data `D` for type `T`.
    ///
    /// Most of the time [`TypeRegistry::register`] can be used instead
    /// to register a type you derived `Reflect` for.
    ///
    /// However, in cases where you want to add a piece of type trait
    /// that was not included in the list of `#[reflect(...)]` type trait in the derive,
    /// or where the type is generic and cannot register e.g.
    /// `ReflectSerialize` unconditionally without knowing the specific type parameters,
    /// this method can be used to insert additional type trait.
    ///
    /// # Panic
    ///
    /// Panic if type `T` is not registered.
    ///
    /// # Example
    /// ```
    /// use voker_reflect::registry::{TypeRegistry, ReflectSerialize, ReflectDeserialize};
    ///
    /// let mut type_registry = TypeRegistry::default();
    /// type_registry
    ///     .register::<Option<String>>()
    ///     .register_type_trait::<Option<String>, ReflectSerialize>()
    ///     .register_type_trait::<Option<String>, ReflectDeserialize>();
    /// ```
    pub fn register_type_trait<T: Typed, D: TypeTrait + FromType<T>>(&mut self) -> &mut Self {
        match self.type_meta_table.get_mut(TypeId::of::<T>()) {
            Some(type_meta) => type_meta.insert_trait(D::from_type()),
            None => panic!(
                "Called `TypeRegistry::register_type_trait`, but the type `{}` of type_trait `{}` without registering",
                T::type_path(),
                core::any::type_name::<D>(),
            ),
        }
        self
    }

    /// Automatically registers all non-generic types annotated with `#[reflect(auto_register)]`
    /// or declared via `impl_auto_register!`.
    ///
    /// This method is equivalent to calling [`register`](Self::register) for each qualifying type.
    /// Repeated calls are cheap and will not insert duplicates.
    ///
    /// ## Return Value
    ///
    /// Returns `true` if automatic registration succeeded on the current platform; otherwise, `false`.
    /// Successful registrations remain `true` on subsequent calls, allowing you to detect platform support.
    ///
    /// ## Feature Dependency
    ///
    /// This method requires the `auto_register` feature. When disabled, it always do nothing and
    /// returns `false`.
    ///
    /// ## Platform Support
    ///
    /// Supported platforms include Linux, macOS, Windows, iOS, Android, and Web, enabled by
    /// the `inventory` crate. On unsupported platforms, this method becomes a no-op.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use std::any::TypeId;
    /// # use voker_reflect::{Reflect, registry::{TypeRegistry, ReflectDefault}};
    /// #[derive(Reflect, Default)]
    /// #[reflect(default, auto_register)]
    /// struct Foo {
    ///     name: Option<String>,
    ///     value: i32,
    /// }
    ///
    /// let mut type_registry = TypeRegistry::empty();
    /// let successful = type_registry.auto_register();
    ///
    /// assert!(successful);
    ///
    /// // Main type is registered
    /// assert!(type_registry.contains(TypeId::of::<Foo>()));
    ///
    /// // Type dependencies are also registered
    /// assert!(type_registry.contains(TypeId::of::<Option<String>>()));
    /// assert!(type_registry.contains(TypeId::of::<i32>()));
    ///
    /// // Associated type trait is available
    /// assert!(type_registry
    ///     .get_type_trait::<ReflectDefault>(TypeId::of::<Foo>())
    ///     .is_some());
    /// ```
    #[cfg_attr(not(feature = "auto_register"), inline(always))]
    pub fn auto_register(&mut self) -> bool {
        crate::cfg::auto_register! {
            if {
                use crate::__macro_exports::auto_register;
                // Reduce the cost of duplicate registrations.
                if self.contains(TypeId::of::<auto_register::__AvailFlag>()) {
                    return true;
                }
                auto_register::__register_types(self);
                self.contains(TypeId::of::<auto_register::__AvailFlag>())
            } else {
                false
            }
        }
    }

    /// Whether the type with given [`TypeId`] has been registered in this registry.
    #[inline]
    pub fn contains(&self, type_id: TypeId) -> bool {
        self.type_meta_table.contains(type_id)
    }

    /// Returns a reference to the [`TypeMeta`] of the type with
    /// the given [`TypeId`].
    ///
    /// If the specified type has not been registered, returns `None`.
    #[inline]
    pub fn get(&self, type_id: TypeId) -> Option<&TypeMeta> {
        self.type_meta_table.get(type_id)
    }

    /// Returns a mutable reference to the [`TypeMeta`] of the type with
    /// the given [`TypeId`].
    ///
    /// If the specified type has not been registered, returns `None`.
    #[inline]
    pub fn get_mut(&mut self, type_id: TypeId) -> Option<&mut TypeMeta> {
        self.type_meta_table.get_mut(type_id)
    }

    /// Returns a reference to the [`TypeMeta`] of the type with
    /// the given [type path].
    ///
    /// If no type with the given type path has been registered, returns `None`.
    ///
    /// [type path]: crate::info::TypePath::type_path
    pub fn get_with_type_path(&self, type_path: &str) -> Option<&TypeMeta> {
        // Manual inline
        match self.type_path_to_id.get(type_path) {
            Some(id) => self.get(*id),
            None => None,
        }
    }

    /// Returns a mutable reference to the [`TypeMeta`] of the type with
    /// the given [type path].
    ///
    /// If no type with the given type path has been registered, returns `None`.
    ///
    /// [type path]: crate::info::TypePath::type_path
    pub fn get_with_type_path_mut(&mut self, type_path: &str) -> Option<&mut TypeMeta> {
        // Manual inline
        match self.type_path_to_id.get(type_path) {
            Some(id) => self.get_mut(*id),
            None => None,
        }
    }

    /// Returns a reference to the [`TypeMeta`] of the type with the given [type name].
    ///
    /// If the type name is ambiguous, or if no type with the given path
    /// has been registered, returns `None`.
    ///
    /// If two different types share the same short type name, short-name lookup is
    /// intentionally disabled for that name. Use
    /// [`is_ambiguous`](Self::is_ambiguous) to detect this case and
    /// [`get_with_type_path`](Self::get_with_type_path) for unambiguous lookup.
    ///
    /// [type name]: crate::info::TypePath::type_name
    pub fn get_with_type_name(&self, type_name: &str) -> Option<&TypeMeta> {
        match self.type_name_to_id.get(type_name) {
            Some(id) => self.get(*id),
            None => None,
        }
    }

    /// Returns a mutable reference to the [`TypeMeta`] of the type with
    /// the given [type name].
    ///
    /// If the type name is ambiguous, or if no type with the given path
    /// has been registered, returns `None`.
    ///
    /// See [`get_with_type_name`](Self::get_with_type_name) for ambiguity behavior.
    ///
    /// [type name]: crate::info::TypePath::type_name
    pub fn get_with_type_name_mut(&mut self, type_name: &str) -> Option<&mut TypeMeta> {
        match self.type_name_to_id.get(type_name) {
            Some(id) => self.get_mut(*id),
            None => None,
        }
    }

    /// Returns `true` if the given [type name] is ambiguous, that is, it matches multiple registered types.
    ///
    /// # Example
    /// ```
    /// # use voker_reflect::registry::TypeRegistry;
    /// # mod foo {
    /// #     use voker_reflect::Reflect;
    /// #     #[derive(Reflect)]
    /// #     pub struct MyType;
    /// # }
    /// # mod bar {
    /// #     use voker_reflect::Reflect;
    /// #     #[derive(Reflect)]
    /// #     pub struct MyType;
    /// # }
    /// let mut type_registry = TypeRegistry::default();
    /// type_registry.register::<foo::MyType>();
    /// type_registry.register::<bar::MyType>();
    /// assert_eq!(type_registry.is_ambiguous("MyType"), true);
    /// ```
    ///
    /// [type name]: crate::info::TypePath::type_name
    pub fn is_ambiguous(&self, type_name: &str) -> bool {
        self.ambiguous_names.contains(type_name)
    }

    /// Returns a reference to the [`TypeTrait`] of type `T` associated with the given [`TypeId`].
    ///
    /// If the specified type has not been registered, or if `T` is not present
    /// in its type registration, returns `None`.
    pub fn get_type_trait<T: TypeTrait>(&self, type_id: TypeId) -> Option<&T> {
        // Manual inline
        match self.get(type_id) {
            Some(type_meta) => type_meta.get_trait::<T>(),
            None => None,
        }
    }

    /// Returns a mutable reference to the [`TypeTrait`] of type `T` associated with the given [`TypeId`].
    ///
    /// If the specified type has not been registered, or if `T` is not present
    /// in its type registration, returns `None`.
    pub fn get_type_trait_mut<T: TypeTrait>(&mut self, type_id: TypeId) -> Option<&mut T> {
        // Manual inline
        match self.get_mut(type_id) {
            Some(type_meta) => type_meta.get_trait_mut::<T>(),
            None => None,
        }
    }

    /// Returns the [`TypeInfo`] associated with the given [`TypeId`].
    ///
    /// If the specified type has not been registered, returns `None`.
    pub fn get_type_info(&self, type_id: TypeId) -> Option<&'static TypeInfo> {
        self.get(type_id).map(TypeMeta::type_info)
    }

    /// Returns an iterator over the [`TypeMeta`]s of the registered types.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &TypeMeta> {
        self.type_meta_table.values()
    }

    /// Returns a mutable iterator over the [`TypeMeta`]s of the registered types.
    pub fn iter_mut(&mut self) -> impl ExactSizeIterator<Item = &mut TypeMeta> {
        self.type_meta_table.values_mut()
    }

    /// Checks to see if the [`TypeTrait`] of type `T` is associated with each registered type,
    /// returning a ([`TypeMeta`], [`TypeTrait`]) iterator for all entries where data of that type was found.
    pub fn iter_with_trait<T: TypeTrait>(&self) -> impl Iterator<Item = (&TypeMeta, &T)> {
        self.type_meta_table.values().filter_map(|item| {
            let type_trait = item.get_trait::<T>();
            type_trait.map(|t| (item, t))
        })
    }
}

// -----------------------------------------------------------------------------
// TypeRegistryArc

use voker_os::sync::{Arc, PoisonError};
use voker_os::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

#[derive(Clone, Default)]
pub struct TypeRegistryArc {
    /// The wrapped [`TypeRegistry`].
    pub internal: Arc<RwLock<TypeRegistry>>,
}

impl TypeRegistryArc {
    /// Takes a read lock on the underlying [`TypeRegistry`].
    pub fn read(&self) -> RwLockReadGuard<'_, TypeRegistry> {
        self.internal.read().unwrap_or_else(PoisonError::into_inner)
    }

    /// Takes a write lock on the underlying [`TypeRegistry`].
    pub fn write(&self) -> RwLockWriteGuard<'_, TypeRegistry> {
        self.internal.write().unwrap_or_else(PoisonError::into_inner)
    }
}

impl core::fmt::Debug for TypeRegistryArc {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.internal
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .type_path_to_id
            .keys()
            .fmt(f)
    }
}

// -----------------------------------------------------------------------------
// tests

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;
    use core::any::TypeId;

    use super::{TypeRegistry, TypeRegistryArc};
    use crate::Reflect;
    use crate::info::TypePath;
    use crate::registry::{ReflectDefault, ReflectFromPtr};

    mod foo {
        use crate::Reflect;

        #[derive(Reflect)]
        pub struct MyType;
    }

    mod bar {
        use crate::Reflect;

        #[derive(Reflect)]
        pub struct MyType;
    }

    #[derive(Reflect, Default)]
    #[reflect(default)]
    struct NeedsDefault {
        value: i32,
    }

    #[test]
    fn lookup_and_ambiguity_checks() {
        let mut registry = TypeRegistry::empty();
        registry.register::<foo::MyType>();
        registry.register::<bar::MyType>();

        assert!(registry.get_with_type_path(foo::MyType::type_path()).is_some());
        assert!(registry.get_with_type_path(bar::MyType::type_path()).is_some());
        assert!(registry.is_ambiguous("MyType"));
        assert!(registry.get_with_type_name("MyType").is_none());
    }

    #[test]
    fn registers_traits() {
        let mut registry = TypeRegistry::default();
        registry.register::<NeedsDefault>();

        assert!(registry.contains(TypeId::of::<NeedsDefault>()));
        assert!(
            registry
                .get_type_trait::<ReflectDefault>(TypeId::of::<NeedsDefault>())
                .is_some()
        );
        assert!(
            registry
                .get_type_trait::<ReflectFromPtr>(TypeId::of::<NeedsDefault>())
                .is_some()
        );

        let with_default: Vec<_> = registry
            .iter_with_trait::<ReflectDefault>()
            .map(|(meta, _)| meta.type_id())
            .collect();
        assert!(with_default.contains(&TypeId::of::<NeedsDefault>()));

        let arc = TypeRegistryArc::default();
        arc.write().register::<NeedsDefault>();
        assert!(arc.read().contains(TypeId::of::<NeedsDefault>()));
    }
}
