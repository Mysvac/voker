use alloc::boxed::Box;
use core::any::{Any, TypeId};
use core::ops::{Deref, DerefMut};

use voker_utils::extra::TypeIdMap;

use crate::Reflect;
use crate::info::{Type, TypeInfo, Typed};
use crate::registry::{TypeRegistry, TypeTrait};

// -----------------------------------------------------------------------------
// TypeMeta

/// Runtime storage for type metadata, registered into the [`TypeRegistry`].
///
/// This includes a [`TypeInfo`] and a [`TypeTrait`] table.
///
/// An instance of `TypeMeta` can be created using the [`TypeMeta::of`]
/// method, but is more often automatically generated using
/// [`#[derive(Reflect)]`](crate::derive::Reflect), which generates
/// an implementation of the [`GetTypeMeta`] trait.
///
/// # Example
///
/// ```
/// # use voker_reflect::registry::{TypeMeta, ReflectDefault, FromType};
/// let mut meta = TypeMeta::of::<String>();
/// meta.insert_trait::<ReflectDefault>(FromType::<String>::from_type());
///
/// let f = meta.get_trait::<ReflectDefault>().unwrap();
/// let s = f.default().take::<String>().unwrap();
///
/// assert_eq!(s, "");
/// ```
///
/// See the [crate-level documentation] for more information on type_meta.
///
/// [crate-level documentation]: crate
pub struct TypeMeta {
    // Access `Type` from `TypeInfo` should judge once reflect kind.
    // We cache the reference to reduce the cost of some methods.
    //
    // We temporarily believe that a little extra memory is worth it.
    ty: &'static Type,
    type_info: &'static TypeInfo,
    trait_table: TypeIdMap<Box<dyn TypeTrait>>,
}

impl TypeMeta {
    /// Create a empty [`TypeMeta`] from a type.
    ///
    /// If you know the number of [`TypeTrait`] in advance,
    /// consider use [`TypeMeta::with_capacity`] for better performence,
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::registry::TypeMeta;
    /// let mut meta = TypeMeta::of::<String>();
    /// ```
    #[inline]
    pub fn of<T: Typed>() -> Self {
        let type_info = T::type_info();
        let ty = type_info.ty();
        Self {
            ty,
            type_info,
            trait_table: TypeIdMap::new(),
        }
    }

    /// Create a empty [`TypeMeta`] from a type with capacity.
    #[inline]
    pub fn with_capacity<T: Typed>(capacity: usize) -> Self {
        let type_info = T::type_info();
        let ty = type_info.ty();
        Self {
            ty,
            type_info,
            trait_table: TypeIdMap::with_capacity(capacity),
        }
    }

    /// Returns the [`TypeInfo`] .
    #[inline(always)]
    pub const fn type_info(&self) -> &'static TypeInfo {
        self.type_info
    }

    /// Returns the [`Type`] .
    ///
    /// Manually impl for static reference.
    #[inline(always)]
    pub const fn ty(&self) -> &'static Type {
        self.ty
    }

    crate::info::impl_type_fn!();

    /// Returns the [`Generics`](crate::info::Generics) .
    #[inline]
    pub const fn generics(&self) -> &'static crate::info::Generics {
        self.type_info.generics()
    }

    /// Return the docs.
    ///
    /// If reflect_docs feature is not enabled, this function always return `None`.
    /// So you can use this without worrying about compilation options.
    #[inline]
    pub const fn docs(&self) -> Option<&'static str> {
        self.type_info.docs()
    }

    /// Returns the [`CustomAttributes`](crate::info::CustomAttributes) .
    #[inline]
    pub fn custom_attributes(&self) -> &'static crate::info::CustomAttributes {
        self.type_info.custom_attributes()
    }

    /// Returns the attribute of type `T`, if present.
    pub fn get_attribute<T: Reflect>(&self) -> Option<&'static T> {
        self.custom_attributes().get::<T>()
    }

    /// Returns the attribute with the given `TypeId`, if present.
    pub fn get_attribute_by_id(
        &self,
        type_id: ::core::any::TypeId,
    ) -> Option<&'static dyn Reflect> {
        self.custom_attributes().get_by_id(type_id)
    }

    /// Returns `true` if it contains the given attribute type.
    pub fn has_attribute<T: Reflect>(&self) -> bool {
        self.custom_attributes().contains::<T>()
    }

    /// Returns `true` if it contains the attribute with the given `TypeId`.
    pub fn has_attribute_by_id(&self, type_id: ::core::any::TypeId) -> bool {
        self.custom_attributes().contains_by_id(type_id)
    }

    /// Insert a new [`TypeTrait`].
    #[inline(always)]
    pub fn insert_trait<T: TypeTrait>(&mut self, data: T) {
        self.insert_trait_by_id(TypeId::of::<T>(), Box::new(data));
    }

    /// Block code inline.
    #[inline(never)]
    fn insert_trait_by_id(&mut self, id: TypeId, val: Box<dyn TypeTrait>) {
        self.trait_table.insert(id, val);
    }

    /// Removes a [`TypeTrait`] from the meta.
    #[inline]
    pub fn remove_trait<T: TypeTrait>(&mut self) -> Option<Box<T>> {
        self.remove_trait_by_id(TypeId::of::<T>())
            .map(|v| <Box<dyn Any>>::downcast::<T>(v).unwrap())
    }

    /// Removes a [`TypeTrait`] from the meta.
    pub fn remove_trait_by_id(&mut self, type_id: TypeId) -> Option<Box<dyn TypeTrait>> {
        self.trait_table.remove(type_id)
    }

    /// Get a [`TypeTrait`] reference, or return `None` if it's doesn't exist.
    #[inline]
    pub fn get_trait<T: TypeTrait>(&self) -> Option<&T> {
        self.get_trait_by_id(TypeId::of::<T>())
            .and_then(<dyn TypeTrait>::downcast_ref)
    }

    /// Get a [`TypeTrait`] reference, or return `None` if it's doesn't exist.
    pub fn get_trait_by_id(&self, type_id: TypeId) -> Option<&dyn TypeTrait> {
        self.trait_table.get(type_id).map(Deref::deref)
    }

    /// Get a mutable [`TypeTrait`] reference, or return `None` if it's doesn't exist.
    #[inline]
    pub fn get_trait_mut<T: TypeTrait>(&mut self) -> Option<&mut T> {
        self.get_trait_mut_by_id(TypeId::of::<T>())
            .and_then(<dyn TypeTrait>::downcast_mut)
    }

    /// Get a mutable [`TypeTrait`] reference, or return `None` if it's doesn't exist.
    pub fn get_trait_mut_by_id(&mut self, type_id: TypeId) -> Option<&mut dyn TypeTrait> {
        self.trait_table.get_mut(type_id).map(DerefMut::deref_mut)
    }

    /// Return true if specific [`TypeTrait`] is exist.
    #[inline]
    pub fn has_trait<T: TypeTrait>(&self) -> bool {
        self.has_trait_by_id(TypeId::of::<T>())
    }

    /// Return true if specific [`TypeTrait`] is exist.
    pub fn has_trait_by_id(&self, type_id: TypeId) -> bool {
        self.trait_table.contains(type_id)
    }

    /// Return the number of [`TypeTrait`].
    #[inline]
    pub fn trait_count(&self) -> usize {
        self.trait_table.len()
    }

    /// An iterator visiting all `TypeId - &dyn TypeTrait` pairs in arbitrary order.
    pub fn trait_iter(&self) -> impl ExactSizeIterator<Item = (TypeId, &dyn TypeTrait)> {
        self.trait_table.iter().map(|(key, val)| (key, val.deref()))
    }

    /// An iterator visiting all `TypeId - &mut dyn TypeTrait` pairs in arbitrary order.
    pub fn trait_iter_mut(
        &mut self,
    ) -> impl ExactSizeIterator<Item = (TypeId, &mut dyn TypeTrait)> {
        self.trait_table
            .iter_mut()
            .map(|(key, val)| (key, val.deref_mut()))
    }
}

impl Clone for TypeMeta {
    fn clone(&self) -> Self {
        let mut new_map = TypeIdMap::with_capacity(self.trait_count());
        for (id, type_trait) in self.trait_table.iter() {
            new_map.insert(id, (**type_trait).clone_type_trait());
        }

        Self {
            trait_table: new_map,
            type_info: self.type_info,
            ty: self.ty,
        }
    }
}

impl core::fmt::Debug for TypeMeta {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TypeMeta")
            .field("type_info", &self.type_info)
            .field("trait_table", &self.trait_table)
            .finish()
    }
}

// -----------------------------------------------------------------------------
// GetTypeMeta

/// A trait which allows a type to generate its [`TypeMeta`]
/// for registration into the [`TypeRegistry`].
///
/// This trait is automatically implemented for items using
/// [`#[derive(Reflect)]`](crate::derive::Reflect).
/// The macro also allows [`TypeTrait`] to be more easily registered.
///
/// # Implementation
///
/// Use [`#[derive(Reflect)]`](crate::derive::Reflect):
///
/// ```
/// use voker_reflect::{Reflect, registry::GetTypeMeta};
///
/// #[derive(Reflect)]
/// struct A;
///
/// let meta = A::get_type_meta();
/// ```
///
/// Add additional [`TypeTrait`]:
///
/// ```
/// use voker_reflect::{derive::{Reflect, reflect_trait}, registry::GetTypeMeta};
///
/// #[reflect_trait]
/// trait MyDisplay {
///     fn display(&self) { /* ... */ }
/// }
///
/// impl MyDisplay for A{}
///
/// #[derive(Reflect)]
/// #[reflect(type_trait = MyDisplayFromReflect)]
/// struct A;
///
/// let meta = A::get_type_meta();
/// assert!(meta.has_trait::<MyDisplayFromReflect>());
/// ```
///
/// See [`derive::reflect_trait`](crate::derive::reflect_trait) for more details.
///
/// ## Manually
///
/// ```
/// use voker_reflect::derive::{Reflect, reflect_trait};
/// use voker_reflect::registry::{GetTypeMeta, FromType, TypeMeta};
///
/// #[reflect_trait]
/// trait MyDisplay {
///     fn display(&self) { /* ... */ }
/// }
///
/// impl MyDisplay for A{}
///
/// #[derive(Reflect)]
/// #[reflect(GetTypeMeta = false)]
/// struct A;
///
/// impl GetTypeMeta for A {
///     fn get_type_meta() -> TypeMeta {
///         let mut meta = TypeMeta::of::<Self>();
///         meta.insert_trait::<MyDisplayFromReflect>(FromType::<Self>::from_type());
///         meta
///     }
/// }
///
/// let meta = A::get_type_meta();
/// assert!(meta.has_trait::<MyDisplayFromReflect>());
/// ```
///
/// [`TypeTrait`]: crate::registry::TypeTrait
/// [crate-level documentation]: crate
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement `GetTypeMeta` so cannot provide type registration information",
    note = "consider annotating `{Self}` with `#[derive(Reflect)]`"
)]
pub trait GetTypeMeta: Typed {
    /// Returns the **default** [`TypeMeta`] for this type.
    fn get_type_meta() -> TypeMeta;

    /// Registers other types needed by this type.
    /// **Allow** not to register oneself.
    fn register_dependencies(_registry: &mut TypeRegistry) {}
}

// -----------------------------------------------------------------------------
// tests

#[cfg(test)]
mod tests {
    use super::TypeMeta;
    use crate::derive::{Reflect, TypePath};
    use crate::registry::{GetTypeMeta, ReflectDefault};
    use alloc::string::String;

    #[derive(Clone, TypePath)]
    struct Counter(u32);

    #[derive(Reflect)]
    #[reflect(@123_u32)]
    struct Tagged;

    #[test]
    fn manages_traits_and_attributes() {
        let mut meta = TypeMeta::with_capacity::<String>(1);
        meta.insert_trait(Counter(1));

        assert!(meta.has_trait::<Counter>());
        assert_eq!(meta.trait_count(), 1);
        assert_eq!(meta.get_trait::<Counter>().unwrap().0, 1);

        meta.get_trait_mut::<Counter>().unwrap().0 = 4;
        assert_eq!(meta.get_trait::<Counter>().unwrap().0, 4);

        let cloned = meta.clone();
        assert_eq!(cloned.get_trait::<Counter>().unwrap().0, 4);

        let removed = meta.remove_trait::<Counter>().unwrap();
        assert_eq!(removed.0, 4);
        assert!(!meta.has_trait::<Counter>());

        let tagged_meta = Tagged::get_type_meta();
        assert_eq!(tagged_meta.get_attribute::<u32>(), Some(&123_u32));
        assert!(!tagged_meta.has_trait::<ReflectDefault>());
    }
}
