use alloc::boxed::Box;
use core::any::{Any, TypeId};
use core::cmp::Ordering;

use crate::info::{DynamicTypePath, DynamicTyped, TypePath, Typed};
use crate::info::{OpaqueInfo, ReflectKind, TypeInfo};
use crate::ops::{ApplyError, ReflectCloneError};
use crate::ops::{ReflectMut, ReflectOwned, ReflectRef};

// -----------------------------------------------------------------------------
// Reflect

/// The foundational trait for runtime reflection in [`voker_reflect`].
///
/// This trait enables dynamic access and modification of data without compile-time
/// type information. It's the core abstraction that powers the entire reflection system.
///
/// # Recommendations
///
/// It's strongly recommended to use [the derive macro for `Reflect`] rather than
/// manually implementing this trait. The derive macro automatically implements
/// this trait along with other useful reflection traits ([`Struct`], [`TupleStruct`],
/// or [`Enum`]) based on the type's structure.
///
/// # Core Functionality
///
/// ## Type Information
///
/// `Reflect` extends [`DynamicTypePath`] and [`DynamicTyped`], providing:
///
/// ```rust
/// # use voker_reflect::{Reflect, info::{DynamicTypePath, DynamicTyped}};
/// let value = 10i32.into_boxed_reflect();
/// let type_path = value.reflect_type_path();   // Gets the type's path
/// let type_info = value.reflect_type_info();   // Gets the type's reflection metadata
/// ```
///
/// ## Type Identification
///
/// `Reflect` implements [`Any`], allowing calls to [`Any::type_id`].
///
/// Note that if [`Any`] is imported in scope, calling `type_id` on containers
/// like [`Box<dyn Reflect>`] will return the container's type ID rather than
/// the contained reflective object's type ID.
///
/// The voker framework prohibits `Box<T>::type_id` via `clippy` lints,
/// preventing such misuse internally. However, framework users must remain
/// vigilant to avoid this error in their own code.
///
/// ```rust
/// use core::any::TypeId;
/// use voker_reflect::Reflect;
///
/// let x: Box<dyn Reflect> = Box::<i32>::new(32_i32);
///
/// assert!((*x).type_id() == TypeId::of::<i32>());      // Preferred method
/// ```
///
/// ## Dynamic Type Representation
///
/// Dynamic types have opaque type information ([`OpaqueInfo`]). Use [`represented_type_info`]
/// to get the type information they represent:
///
/// ```rust
/// # use voker_reflect::Reflect;
/// let dynamic_array = [1_i32, 2, 3, 4, 5].to_dynamic();
/// let info = dynamic_array.represented_type_info().unwrap();
/// assert!(info.type_is::<[i32; 5]>());  // Represents [i32; 5]
/// ```
///
/// For non-dynamic type, it's equal to [`reflect_type_info`].
///
/// ## Type Casting
///
/// Use [`reflect_ref`], [`reflect_mut`], and [`reflect_owned`] to cast to reflection
/// subtypes ([`Struct`], [`TupleStruct`], [`Enum`], etc.):
///
/// ```rust
/// # use voker_reflect::{Reflect, ops::List};
/// let vec = vec![1, 2, 3].into_boxed_reflect();
/// let list: Box<dyn List> = vec.reflect_owned().into_list().unwrap();
/// ```
///
/// Use `downcast_ref`, `downcast_mut`, and `downcast` for concrete type conversion:
///
/// ```rust
/// # use voker_reflect::Reflect;
/// let x: Box<dyn Reflect> = 10.into_boxed_reflect();
/// let y = x.downcast_ref::<i32>().unwrap();
/// assert_eq!(*y, 10);
/// ```
///
/// Cast choice guideline:
/// - Prefer `downcast_ref/downcast_mut/downcast` when you need one concrete type.
/// - Prefer `reflect_ref/reflect_mut/reflect_owned` when you need kind-driven generic logic
///   (for example: struct/list/enum traversal).
///
/// ## Borrowing and Ownership Flow
///
/// The three kind-cast methods mirror ownership exactly:
/// - [`reflect_ref`](Reflect::reflect_ref): borrows immutably for read-only traversal.
/// - [`reflect_mut`](Reflect::reflect_mut): borrows mutably for in-place updates.
/// - [`reflect_owned`](Reflect::reflect_owned): consumes `Box<Self>` and transfers ownership
///   into an owned reflect kind wrapper.
///
/// This keeps conversions explicit and avoids accidental allocations when only borrowed
/// access is needed.
///
/// # Implementation Guide
///
/// ## Using the Derive Macro (Recommended)
///
/// See [derive::Reflect](crate::derive::Reflect) for comprehensive documentation.
///
/// ## Manual Implementation
///
/// Some methods have standard implementations:
///
/// ```rust, ignore
/// fn reflect_assign(&mut self, value: Box<dyn Reflect>) -> Result<(), Box<dyn Reflect>> {
///     *self = value.take::<Self>()?;  // Extract Self from Box<dyn Reflect>
///     Ok(())
/// }
///
/// fn reflect_kind(&self) -> ReflectKind {
///     ReflectKind::Kind  // e.g., ReflectKind::Struct, ReflectKind::Enum
/// }
///
/// fn reflect_ref(&self) -> ReflectRef<'_> {
///     ReflectRef::Kind(self)  // Construct appropriate ReflectRef variant
/// }
///
/// fn reflect_mut(&mut self) -> ReflectMut<'_> {
///     ReflectMut::Kind(self)  // Construct appropriate ReflectMut variant
/// }
///
/// fn reflect_owned(self: Box<Self>) -> ReflectOwned {
///     ReflectOwned::Kind(self)  // Construct appropriate ReflectOwned variant
/// }
///
/// fn to_dynamic(&self) -> Box<dyn Reflect> {
///     // Convert to corresponding dynamic type
///     Box::new(<Self as Kind>::to_dynamic_kind(self))
///     // for example: `Box::new(<Self as Struct>::to_dynamic_struct(self))`
/// }
/// ```
///
/// ### Helper Functions
///
/// For non-opaque types, use helper functions in [`voker_reflect::impls`]:
///
/// - [`struct_apply`](crate::impls::struct_apply)
/// - [`struct_eq`](crate::impls::struct_eq)
/// - [`struct_cmp`](crate::impls::struct_cmp)
/// - [`struct_debug`](crate::impls::struct_debug)
/// - [`struct_hash`](crate::impls::struct_hash)
/// - Similar functions for tuples, enums, arrays ...
///
/// Only [`reflect_clone`] must be implemented manually (use `self.clone()` for
/// cloneable types).
///
/// ### Opaque Types
///
/// Opaque types require manual implementation of all methods. Consider implementing
/// one of the reflection subtraits ([`Struct`], [`TupleStruct`], [`Enum`]) for
/// better integration.
///
/// [`reflect_cmp`]: Reflect::reflect_cmp
/// [`reflect_eq`]: Reflect::reflect_eq
/// [`reflect_hash`]: Reflect::reflect_hash
/// [`reflect_debug`]: Reflect::reflect_debug
/// [`reflect_clone`]: Reflect::reflect_clone
/// [`apply`]: Reflect::apply
/// [`to_dynamic`]: Reflect::to_dynamic
/// [`reflect_ref`]: Reflect::reflect_ref
/// [`reflect_mut`]: Reflect::reflect_mut
/// [`reflect_owned`]: Reflect::reflect_owned
/// [`reflect_type_info`]: crate::info::DynamicTyped::reflect_type_info
/// [`represented_type_info`]: Reflect::represented_type_info
/// [`voker_reflect`]: crate
/// [the derive macro for `Reflect`]: crate::derive::Reflect
/// [`Struct`]: crate::ops::Struct
/// [`TupleStruct`]: crate::ops::TupleStruct
/// [`Enum`]: crate::ops::Enum
/// [`OpaqueInfo`]: crate::info::OpaqueInfo
/// [`DynamicTypePath`]: crate::info::DynamicTypePath
/// [`DynamicTyped`]: crate::info::DynamicTyped
/// [`Any`]: core::any::Any
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement `Reflect`",
    note = "consider annotating `{Self}` with `#[derive(Reflect)]`"
)]
pub trait Reflect: DynamicTypePath + DynamicTyped + Send + Sync + Any {
    /// Casts this type to a fully-reflected value.
    ///
    /// # Example
    ///
    /// ```
    /// use voker_reflect::Reflect;
    ///
    /// let x = 32;
    /// let r: &dyn Reflect = x.as_reflect();
    /// // Equal to this:
    /// // let r: &dyn Reflect = &x;
    /// ```
    #[inline(always)]
    fn as_reflect(&self) -> &dyn Reflect
    where
        Self: Sized,
    {
        self
    }

    /// Casts this type to a mutable, fully-reflected value.
    ///
    /// # Example
    ///
    /// ```
    /// use voker_reflect::Reflect;
    ///
    /// let mut x = 32;
    /// let r: &mut dyn Reflect = x.as_mut_reflect();
    /// // Equal to this:
    /// // let r: &mut dyn Reflect = &mut x;
    /// ```
    #[inline(always)]
    fn as_mut_reflect(&mut self) -> &mut dyn Reflect
    where
        Self: Sized,
    {
        self
    }

    /// Casts this type to a boxed, fully-reflected value.
    ///
    /// # Example
    ///
    /// ```
    /// use voker_reflect::Reflect;
    ///
    /// let r = 32.into_boxed_reflect();
    /// // Equal to this:
    /// // let r = Box::new(32) as Box<dyn Reflect>;
    /// ```
    #[inline(always)]
    fn into_boxed_reflect(self) -> Box<dyn Reflect>
    where
        Self: Sized,
    {
        Box::new(self)
    }

    /// Indicates whether or not this type is a _dynamic_ data type.
    ///
    /// Normally, All other types should return false,
    /// meaning there is no need to implement it.
    #[inline]
    fn is_dynamic(&self) -> bool {
        false
    }

    /// Returns the [`TypeInfo`] of the type **represented** by this value.
    ///
    /// For most types, this simply returns their own [`TypeInfo`], just like [`reflect_type_info`].
    ///
    /// However, for dynamic types, such as [`DynamicStruct`] or [`DynamicList`],
    /// this will return the type they represent
    /// (or `None` if they don't represent any particular type).
    ///
    /// [`reflect_type_info`]: crate::info::DynamicTyped
    /// [`DynamicStruct`]: crate::ops::DynamicStruct
    /// [`DynamicList`]: crate::ops::DynamicList
    /// [`TypeRegistry::get_type_info`]: crate::registry::TypeRegistry::get_type_info
    #[inline]
    fn represented_type_info(&self) -> Option<&'static TypeInfo> {
        Some(self.reflect_type_info())
    }

    /// Returns a pure enumeration of ["kinds"](ReflectKind) of type.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, info::ReflectKind};
    /// let vec = vec![1, 2, 3].into_boxed_reflect();
    ///
    /// assert_eq!(vec.reflect_kind(), ReflectKind::List);
    /// ```
    fn reflect_kind(&self) -> ReflectKind;

    /// Returns an immutable enumeration of ["kinds"](ReflectRef) of type.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::List};
    /// let vec = vec![1, 2, 3].into_boxed_reflect();
    ///
    /// let vec_mut: &dyn List = vec.reflect_ref().as_list().unwrap();
    /// ```
    fn reflect_ref(&self) -> ReflectRef<'_>;

    /// Returns a mutable enumeration of ["kinds"](ReflectMut) of type.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::List};
    /// let mut vec = vec![1, 2, 3].into_boxed_reflect();
    ///
    /// let vec_mut: &mut dyn List = vec.reflect_mut().as_list().unwrap();
    /// ```
    fn reflect_mut(&mut self) -> ReflectMut<'_>;

    /// Returns an owned enumeration of ["kinds"](ReflectOwned) of type.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::List};
    /// let vec = vec![1, 2, 3].into_boxed_reflect();
    ///
    /// let vec: Box<dyn List> = vec.reflect_owned().into_list().unwrap();
    /// ```
    fn reflect_owned(self: Box<Self>) -> ReflectOwned;

    /// Converts this reflected value into its dynamic representation based on its [kind].
    ///
    /// For example, a [`List`] type will internally invoke [`List::to_dynamic_list`], returning [`DynamicList`].
    /// A [`Struct`] type will invoke [`Struct::to_dynamic_struct`], returning [`DynamicStruct`].
    /// And so on.
    ///
    /// If the [kind] is [opaque], then the value will attempt to be cloned directly via [`reflect_clone`],
    /// since opaque types do not have any standard dynamic representation.
    ///
    /// To attempt to clone the value directly such that it returns a concrete instance of this type,
    /// use [`reflect_clone`].
    ///
    /// # Panics
    ///
    /// This method will panic if the [kind] is [opaque] and the call to [`reflect_clone`] fails.
    ///
    /// By default, [`#[derive(Reflect)]`](crate::derive::Reflect) requires the
    /// `#[reflect(Clone)]` flag for opaque types, so this function rarely panics.
    ///
    /// # Example
    ///
    /// ```
    /// # use voker_reflect::Reflect;
    /// let value = (1, true, 3.14);
    /// let dynamic_value = value.to_dynamic();
    /// assert!(dynamic_value.is_dynamic())
    /// ```
    ///
    /// [kind]: Reflect::reflect_kind
    /// [`List`]: crate::ops::List
    /// [`List::to_dynamic_list`]: crate::ops::List::to_dynamic_list
    /// [`DynamicList`]: crate::ops::DynamicList
    /// [`Struct`]: crate::ops::Struct
    /// [`Struct::to_dynamic_struct`]: crate::ops::Struct::to_dynamic_struct
    /// [`DynamicStruct`]: crate::ops::DynamicStruct
    /// [opaque]: crate::info::ReflectKind::Opaque
    /// [`reflect_clone`]: Reflect::reflect_clone
    #[inline]
    fn to_dynamic(&self) -> Box<dyn Reflect> {
        // Separate to reduce compilation complexity.
        #[inline(never)]
        fn to_dynamic_internal(reflect_ref: &ReflectRef<'_>) -> Box<dyn Reflect> {
            match *reflect_ref {
                ReflectRef::Struct(val) => Box::new(val.to_dynamic_struct()),
                ReflectRef::TupleStruct(val) => Box::new(val.to_dynamic_tuple_struct()),
                ReflectRef::Tuple(val) => Box::new(val.to_dynamic_tuple()),
                ReflectRef::List(val) => Box::new(val.to_dynamic_list()),
                ReflectRef::Array(val) => Box::new(val.to_dynamic_array()),
                ReflectRef::Map(val) => Box::new(val.to_dynamic_map()),
                ReflectRef::Set(val) => Box::new(val.to_dynamic_set()),
                ReflectRef::Enum(val) => Box::new(val.to_dynamic_enum()),
                ReflectRef::Opaque(val) => val.reflect_clone().unwrap(),
            }
        }

        to_dynamic_internal(&self.reflect_ref())
    }

    /// Tries to apply a reflected value to this value.
    ///
    /// # Apply Rules
    ///
    /// If `self.type_id` == `value.type_id`:
    ///
    /// - If the type supports `Clone`, try `Reflect::downcast_ref` + `Clone::clone` + assignment.
    /// - Otherwise, try `Reflect::reflect_clone` + `Reflect::take` + assignment.
    ///
    /// Otherwise, dispatch to one of the following helpers based on [`ReflectKind`]:
    ///
    /// - [`crate::impls::array_apply`]
    /// - [`crate::impls::list_apply`]
    /// - [`crate::impls::struct_apply`]
    /// - [`crate::impls::tuple_struct_apply`]
    /// - [`crate::impls::tuple_apply`]
    /// - [`crate::impls::enum_apply`]
    /// - [`crate::impls::set_apply`]
    /// - [`crate::impls::map_apply`]
    ///
    /// The only special kind is `Enum`: values of the same type but different variants
    /// cannot `apply` through `enum_apply` directly.
    /// The default implementation may depend on [`FromReflect`](crate::FromReflect).
    ///
    /// # Fail Reason
    /// - Different [`ReflectKind`] values.
    /// - Different item/field counts in `Array`, `Tuple`, `TupleStruct`, and `Enum` tuple variants.
    /// - An incompatible type encountered during application.
    /// - Opaque types that do not support `Clone` or `reflect_clone`.
    ///
    /// # Handling Errors
    ///
    /// This function may leave `self` in a partially mutated state if an error is encountered.
    /// Consider keeping a cloned backup if you need rollback semantics.
    ///
    /// ```
    /// # use voker_reflect::Reflect;
    /// # #[derive(Reflect, Clone, Debug, PartialEq)]
    /// # struct Value { a: i32, b: bool }
    /// let mut target = Value { a: 1, b: false };
    /// let backup = target.clone();
    ///
    /// if target.apply(&Value { a: 9, b: true }).is_err() {
    ///     // Manual rollback policy decided by the caller.
    ///     target = backup;
    /// }
    /// ```
    fn apply(&mut self, value: &dyn Reflect) -> Result<(), ApplyError>;

    /// Performs a type-checked assignment of a reflected value to this value.
    ///
    /// This is type strict but fast; to allow compatible-but-not-identical inputs,
    /// use [`Reflect::apply`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::Reflect;
    /// let data = vec![1_i32, 2_i32, 3_i32].into_boxed_reflect();
    /// let mut vec = Vec::<i32>::new();
    ///
    /// vec.reflect_assign(data).unwrap();
    /// assert_eq!(vec, [1, 2, 3]);
    /// ```
    fn reflect_assign(&mut self, value: Box<dyn Reflect>) -> Result<(), Box<dyn Reflect>>;

    /// Attempts to clone `Self` using reflection.
    ///
    /// Unlike [`to_dynamic`], which generally returns a dynamic representation of `Self`,
    /// this method attempts to create a clone of `Self` directly, if possible.
    ///
    /// This function normally succeeds, except for certain types that explicitly prohibit cloning.
    /// But if the clone cannot be performed, an appropriate [`ReflectCloneError`] is returned.
    ///
    /// Note that when cloning successfully, the returned value
    /// must have the same type, otherwise some APIs may panic.
    ///
    /// # Example
    ///
    /// ```
    /// # use voker_reflect::Reflect;
    /// let value = (1, true, 3.14);
    /// let cloned = value.reflect_clone().unwrap();
    /// assert!(cloned.is::<(i32, bool, f64)>())
    /// ```
    ///
    /// # Rules
    ///
    /// 1. If the `reflect(Clone)` flag is enabled, this function will call [`Clone::clone`] directly.
    /// 2. Otherwise:
    ///     1. If the type is a unit struct without the `reflect(Opaque)` flag, create a new value directly.
    ///     2. If the type is not opaque, try to clone its fields and construct a new value.
    ///     3. Return `Err`.
    ///
    /// Therefore, it's generally recommended to implement [`Clone`] for your type and
    /// mark it with the `#[reflect(Clone)]` attribute.
    ///
    /// ```
    /// # use voker_reflect::Reflect;
    /// #[derive(Reflect, Clone)]
    /// #[reflect(Clone)]
    /// struct A { /* ... */ }
    /// ```
    ///
    /// By default, [`#[derive(Reflect)]`](crate::derive::Reflect) requires
    /// the `reflect(Clone)` flag for Opaque type, so this function always succeeds.
    ///
    /// [`to_dynamic`]: Reflect::to_dynamic
    fn reflect_clone(&self) -> Result<Box<dyn Reflect>, ReflectCloneError>;

    /// Returns a "partial equality" comparison result.
    ///
    /// If the underlying type does not support equality testing, returns `None`.
    ///
    /// In the default implementation, this always returns `None` for opaque types,
    /// while unit structs are compared by checking their type IDs directly.
    ///
    /// However, for composite types, this performs a field-by-field comparison
    /// using `reflect_eq`, which may not be efficient.
    ///
    /// See:
    /// - [`crate::impls::array_eq`]
    /// - [`crate::impls::list_eq`]
    /// - [`crate::impls::struct_eq`]
    /// - [`crate::impls::tuple_struct_eq`]
    /// - [`crate::impls::tuple_eq`]
    /// - [`crate::impls::enum_eq`]
    /// - [`crate::impls::set_eq`]
    /// - [`crate::impls::map_eq`]
    ///
    /// If the type implements [`PartialEq`], consider marking it with the
    /// `#[reflect(PartialEq)]` attribute. When this attribute is present,
    /// the function uses the type's own implementation instead, and types that
    /// differ immediately return `Some(false)`.
    ///
    /// ```
    /// use voker_reflect::Reflect;
    ///
    /// #[derive(Reflect, PartialEq)]
    /// #[reflect(PartialEq)]
    /// struct A { /* ... */ }
    /// ```
    #[inline]
    fn reflect_eq(&self, _other: &dyn Reflect) -> Option<bool> {
        // Only Inline for default implement
        None
    }

    /// Returns a "partial compare" result.
    ///
    /// If the underlying type does not support PartialOrd, returns `None`.
    ///
    /// In the default implementation, this always returns `None` for opaque types,
    /// while unit structs are compared by checking their type IDs directly.
    ///
    /// However, for composite types, this performs a field-by-field comparison
    /// using `reflect_cmp`, which may not be efficient.
    ///
    /// See:
    /// - [`crate::impls::array_cmp`]
    /// - [`crate::impls::list_cmp`]
    /// - [`crate::impls::struct_cmp`]
    /// - [`crate::impls::tuple_struct_cmp`]
    /// - [`crate::impls::tuple_cmp`]
    /// - [`crate::impls::enum_cmp`]
    /// - [`crate::impls::set_cmp`]
    /// - [`crate::impls::map_cmp`]
    ///
    /// If the type implements [`PartialOrd`], consider marking it with the
    /// `#[reflect(PartialOrd)]` attribute. When this attribute is present,
    /// the function uses the type's own implementation instead, and types that
    /// differ immediately return `None`.
    ///
    /// ```
    /// use voker_reflect::Reflect;
    ///
    /// #[derive(Reflect, PartialOrd, PartialEq)]
    /// #[reflect(PartialOrd)]
    /// struct A { /* ... */ }
    /// ```
    fn reflect_cmp(&self, _other: &dyn Reflect) -> Option<Ordering> {
        None
    }

    /// Returns a hash of the value, may differ from [`core::hash::Hash`].
    ///
    /// We use [`reflect_hasher`](crate::reflect_hasher) to ensure that the hash
    /// result of the program running multiple times is the same for the same data.
    ///
    /// In the default implementation, this always returns `None` for opaque types,
    /// while unit structs compute their hash by directly hashing the [`TypeId`].
    ///
    /// For composite types, however, this performs a field-by-field hash using
    /// `reflect_hash`, which may be inefficient.
    ///
    /// See:
    /// - [`crate::impls::array_hash`]
    /// - [`crate::impls::list_hash`]
    /// - [`crate::impls::struct_hash`]
    /// - [`crate::impls::tuple_struct_hash`]
    /// - [`crate::impls::tuple_hash`]
    /// - [`crate::impls::enum_hash`]
    /// - [`crate::impls::set_hash`]
    /// - [`crate::impls::map_hash`]
    ///
    /// If the type implements [`Hash`](core::hash::Hash), consider annotating it with the
    /// `#[reflect(Hash)]` attribute to make this function use the [`Hash`](core::hash::Hash)'s
    /// implementation instead.
    ///
    /// ```
    /// use voker_reflect::Reflect;
    ///
    /// #[derive(Reflect, Hash)]
    /// #[reflect(Hash)]
    /// struct A { /* ... */ }
    /// ```
    #[inline]
    fn reflect_hash(&self) -> Option<u64> {
        None
    }

    /// Debug formatter for the value.
    ///
    /// For opaque type, this function will write `"Opaque(type_path)"` by default.
    ///
    /// For other type, see:
    /// - [`crate::impls::array_debug`]
    /// - [`crate::impls::list_debug`]
    /// - [`crate::impls::struct_debug`]
    /// - [`crate::impls::tuple_struct_debug`]
    /// - [`crate::impls::tuple_debug`]
    /// - [`crate::impls::enum_debug`]
    /// - [`crate::impls::set_debug`]
    /// - [`crate::impls::map_debug`]
    ///
    /// If the type implements [`Debug`](core::fmt::Debug), consider annotating it with the
    /// `#[reflect(Debug)]` attribute to make this function use the [`Debug`](core::fmt::Debug)'s
    /// implementation instead.
    ///
    /// ```
    /// use voker_reflect::Reflect;
    ///
    /// #[derive(Reflect, Debug)]
    /// #[reflect(Debug)]
    /// struct A { /* ... */ }
    /// ```
    ///
    /// [`List`]: crate::ops::List
    /// [`Map`]: crate::ops::Map
    /// [type path]: TypePath::type_path
    #[inline]
    fn reflect_debug(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        // Separate to reduce compilation complexity.
        #[inline(never)]
        fn reflect_debug_internal(
            reflect_ref: &ReflectRef<'_>,
            f: &mut core::fmt::Formatter,
        ) -> core::fmt::Result {
            use crate::impls;

            match *reflect_ref {
                ReflectRef::Struct(data) => impls::struct_debug(data, f),
                ReflectRef::TupleStruct(data) => impls::tuple_struct_debug(data, f),
                ReflectRef::Tuple(data) => impls::tuple_debug(data, f),
                ReflectRef::List(data) => impls::list_debug(data, f),
                ReflectRef::Array(data) => impls::array_debug(data, f),
                ReflectRef::Map(data) => impls::map_debug(data, f),
                ReflectRef::Set(data) => impls::set_debug(data, f),
                ReflectRef::Enum(data) => impls::enum_debug(data, f),
                ReflectRef::Opaque(data) => write!(f, "Opaque({})", data.reflect_type_path()),
            }
        }

        reflect_debug_internal(&self.reflect_ref(), f)
    }
}

impl dyn Reflect {
    /// Returns `true` if the underlying value is of type `T`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::Reflect;
    /// let x: Box<dyn Reflect> = 10.into_boxed_reflect();
    ///
    /// assert!(x.is::<i32>());
    /// ```
    #[inline(always)]
    pub fn is<T: Any>(&self) -> bool {
        // Any::Type_id(self)
        self.type_id() == TypeId::of::<T>()
    }

    /// Returns `true` if the value represents type `T`,  or `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::DynamicList, info::Typed};
    /// let x: Box<dyn Reflect> = 10.into_boxed_reflect();
    /// assert!(x.is::<i32>());
    /// assert!(x.represents::<i32>());
    ///
    /// let mut dynamic = DynamicList::new();
    /// dynamic.set_type_info(Some(<Vec<i32> as Typed>::type_info()));
    /// let d = dynamic.as_reflect();
    ///
    /// assert!( !d.is::<Vec<i32>>() );
    /// assert!( d.represents::<Vec<i32>>() );
    /// ```
    #[inline]
    pub fn represents<T: Any>(&self) -> bool {
        match self.represented_type_info() {
            Some(t) => t.type_id() == TypeId::of::<T>(),
            None => false,
        }
    }

    /// Downcasts the value to type `T` by reference.
    ///
    /// If the underlying value is not of type `T`, returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::Reflect;
    /// let x: Box<dyn Reflect> = 10.into_boxed_reflect();
    ///
    /// let y = x.downcast_ref::<i32>().unwrap();
    /// assert_eq!(*y, 10);
    /// ```
    #[inline]
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        <dyn Any>::downcast_ref(self)
    }

    /// Downcasts the value to type `T` by mutable reference.
    ///
    /// If the underlying value is not of type `T`, returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::Reflect;
    /// let mut x: Box<dyn Reflect> = 10.into_boxed_reflect();
    ///
    /// let y = x.downcast_mut::<i32>().unwrap();
    /// *y += 2;
    ///
    /// assert_eq!(*y, 12);
    /// ```
    #[inline]
    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        <dyn Any>::downcast_mut(self)
    }

    /// Downcasts the value to type `T`, consuming the trait object.
    ///
    /// If the underlying value is not of type `T`, returns `Err(self)`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::Reflect;
    /// let x: Box<dyn Reflect> = 10.into_boxed_reflect();
    ///
    /// let x: Box<i32> = x.downcast::<i32>().unwrap();
    /// assert_eq!(*x, 10);
    /// ```
    #[inline]
    pub fn downcast<T: Any>(self: Box<dyn Reflect>) -> Result<Box<T>, Box<dyn Reflect>> {
        if self.is::<T>() {
            // TODO: replace to `downcast_uncheck` when it's stable,
            #[expect(unsafe_code, reason = "type is already checked")]
            Ok(unsafe { <Box<dyn Any>>::downcast::<T>(self).unwrap_unchecked() })
        } else {
            Err(self)
        }
    }

    /// Downcasts the value to type `T`, unboxing and consuming the trait object.
    ///
    /// If the underlying value is not of type `T`, returns `Err(self)`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::Reflect;
    /// let x: Box<dyn Reflect> = 10.into_boxed_reflect();
    ///
    /// let x = x.take::<i32>().unwrap();
    /// assert_eq!(x, 10);
    /// ```
    #[inline]
    pub fn take<T: Any>(self: Box<dyn Reflect>) -> Result<T, Box<dyn Reflect>> {
        if self.is::<T>() {
            // TODO: replace to `downcast_uncheck` if it's unstable,
            #[expect(unsafe_code, reason = "type is already checked")]
            Ok(unsafe { *<Box<dyn Any>>::downcast::<T>(self).unwrap_unchecked() })
        } else {
            Err(self)
        }
    }
}

impl core::fmt::Debug for dyn Reflect {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.reflect_debug(f)
    }
}

impl TypePath for dyn Reflect {
    #[inline]
    fn type_path() -> &'static str {
        "dyn voker_reflect::Reflect"
    }
    #[inline]
    fn type_name() -> &'static str {
        "dyn Reflect"
    }
    #[inline]
    fn type_ident() -> &'static str {
        "dyn Reflect"
    }
}

impl Typed for dyn Reflect {
    /// This is the [`TypeInfo`] of [`dyn Reflect`],
    /// not the [`TypeInfo`] of the underlying data!!!!
    ///
    /// Use [`DynamicTyped::reflect_type_info`] to get underlying [`TypeInfo`].
    ///
    /// [`dyn Reflect`]: crate::Reflect
    fn type_info() -> &'static TypeInfo {
        static INFO: TypeInfo = TypeInfo::Opaque(OpaqueInfo::new::<dyn Reflect>());
        &INFO
    }
}

// -----------------------------------------------------------------------------
// Auxiliary macro

/// Implement some common methos like `reflect_kind` and `reflect_ref`.
macro_rules! impl_reflect_cast_fn {
    ($kind:ident) => {
        fn reflect_assign(
            &mut self,
            value: ::alloc::boxed::Box<dyn $crate::Reflect>,
        ) -> Result<(), ::alloc::boxed::Box<dyn $crate::Reflect>> {
            *self = value.take::<Self>()?;
            Ok(())
        }

        #[inline]
        fn reflect_kind(&self) -> $crate::info::ReflectKind {
            $crate::info::ReflectKind::$kind
        }

        #[inline]
        fn reflect_ref(&self) -> $crate::ops::ReflectRef<'_> {
            $crate::ops::ReflectRef::$kind(self)
        }

        #[inline]
        fn reflect_mut(&mut self) -> $crate::ops::ReflectMut<'_> {
            $crate::ops::ReflectMut::$kind(self)
        }

        #[inline]
        fn reflect_owned(self: ::alloc::boxed::Box<Self>) -> $crate::ops::ReflectOwned {
            $crate::ops::ReflectOwned::$kind(self)
        }
    };
}

pub(crate) use impl_reflect_cast_fn;

// -----------------------------------------------------------------------------
// tests

#[cfg(test)]
mod tests {
    use crate::info::TypePath;
    use crate::{FromReflect, Reflect};

    #[derive(Reflect, Clone, PartialEq, Debug)]
    #[reflect(Clone)]
    struct Sample {
        value: i32,
    }

    #[test]
    fn core_methods() {
        let mut value = Sample { value: 1 };

        assert_eq!(value.as_reflect().reflect_type_path(), Sample::type_path());

        value.apply(&Sample { value: 2 }).unwrap();
        assert_eq!(value.value, 2);

        let boxed = value.clone().into_boxed_reflect();
        let mut target = Sample { value: 0 };
        target.reflect_assign(boxed).unwrap();
        assert_eq!(target.value, 2);

        let dynamic = target.to_dynamic();
        assert!(dynamic.is_dynamic());
        assert!(dynamic.represents::<Sample>());

        let cloned = target.reflect_clone().unwrap().take::<Sample>().unwrap();
        assert_eq!(cloned, target);

        let rebuilt = Sample::from_reflect(dynamic.as_ref()).unwrap();
        assert_eq!(rebuilt, target);
    }
}
