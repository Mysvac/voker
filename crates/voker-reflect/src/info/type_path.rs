use core::any::{Any, TypeId};

// -----------------------------------------------------------------------------
// TypePath

/// A static accessor to type paths and names.
///
/// Provide a stable and flexible alternative to [`core::any::type_name`]
/// that works across compiler versions and survives code refactoring.
///
/// # Methods
///
/// - [`type_path`]: The unique identifier of the type, cannot be duplicated.
/// - [`type_name`]: Type name without module path, may be duplicated.
/// - [`type_ident`]: The shortest type name without module path and generics.
/// - [`module_path`]: Optional module path.
///
/// We did not provide `crate_name` in `TypePath`, this can save some memory.
///
/// But we provide `crate_name` function in [`TypePathTable`] and other type info structs,
/// which obtain crate name from module path.
///
/// We guarantee that these names do not have the prefix `::`.
/// Users should also ensure this when manually implementing it.
///
/// # Implementation
///
/// ## derive macro
///
/// [`#[derive(TypePath)`](crate::derive::TypePath): only implement `TypePath` trait.
///
/// ```
/// use voker_reflect::derive::TypePath;
///
/// // This type path will not change with compiler versions or recompiles,
/// // although it will not be the same if the definition is moved.
/// #[derive(TypePath)]
/// struct NonStableTypePath;
///
/// // This type path will never change, even if the definition is moved.
/// #[derive(TypePath)]
/// #[reflect(type_path = "my_crate::foo::StableTypePath")]
/// struct StableTypePath;
///
/// // Type paths can have any number of path segments.
/// // the last segment will be considered as type_name/type_ident.
/// #[derive(TypePath)]
/// #[reflect(type_path = "my_crate::foo::bar::baz::DeeplyNestedTypePath")]
/// struct DeeplyNestedTypePath;
///
/// // Generics are also supported, will be recognized by macro automatically.
/// // Should not not manually mark it.
/// #[derive(TypePath)]
/// #[reflect(type_path = "my_crate::foo::StableGenericTypePath")]
/// struct StableGenericTypePath<T, const N: usize>([T; N]);
/// ```
///
/// [`#[derive(Reflect)`](crate::derive::Reflect): impl full reflect, including `TypePath` trait.
///
/// ```
/// use voker_reflect::Reflect;
///
/// // just like `#[derive(TypePath)]`
/// #[derive(Reflect)]
/// struct DefaultImpl;
///
/// #[derive(Reflect)]
/// #[reflect(type_path = "my_crate::foo::CustomImpl")]
/// struct CustomImpl;
///
/// // All other trait can be disabled, only implementing TypePath.
/// // this is equal to #[derive(TypePath)]
/// #[derive(Reflect)]
/// #[reflect(Typed = false, Reflect = false)]
/// #[reflect(FromReflect = false, GetTypeMeta = false)]
/// #[reflect(Struct = false, TupleStruct = false, Enum = false)] // optional
/// struct TypePathOnly;
/// ```
///
/// ## impl for foreign type
///
/// Use [`impl_type_path!`](crate::derive::impl_type_path) macro:
///
/// ```ignore
/// use voker_reflect::derive::impl_type_path;
///
/// // Impl for primitive type, if it's necessary.
/// impl_type_path!(Int);
///
/// // Impl for specified foreign type, with prefix `::`.
/// impl_type_path!(::alloc::string::String);
/// // The prefix `::` is necessary, it indicates that it's a complete path.
/// // Although it will be removed by macro.
///
/// // Generics are also supported.
/// impl_type_path!(::utils::One<T>);
///
/// // Custom module path for specified type.
/// // then, it's type_path is `core::time::Instant`
/// impl_type_path!((in core::time) Instant);
///
/// // Custom module and ident for specified type.
/// // then, it's type_path is `core::time::Ins`
/// impl_type_path!((in core::time as Ins) Instant);
/// ```
///
/// ## Manually
///
/// Users should ensure these names do not have the prefix `::`
/// when manually implementing `TypePath`.
///
/// For non generic types, implementation is simple.
///
/// ```
/// use voker_reflect::info::TypePath;
///
/// struct Foo;
///
/// impl TypePath for Foo {
///     fn type_path() -> &'static str { "my_crate::foo::Foo" }
///     fn type_name() -> &'static str { "Foo" }
///     fn type_ident() -> &'static str { "Foo" }
///     fn module_path() -> Option<&'static str> { Some("my_crate::foo") }
/// }
/// ```
///
/// For generic types, we provide [`GenericTypePathCell`] to simplify it.
///
/// ```
/// use voker_reflect::info::TypePath;
/// use voker_reflect::impls::{concat, GenericTypePathCell};
///
/// struct Foo<T>(T);
///
/// impl<T: TypePath> TypePath for Foo<T> {
///     fn type_path() -> &'static str {
///         static CELL: GenericTypePathCell = GenericTypePathCell::new();
///         CELL.get_or_insert::<Self>(||{
///             concat(&["my_crate::foo::Foo", "<", T::type_path(), ">"])
///         })
///     }
///     fn type_name() -> &'static str {
///         static CELL: GenericTypePathCell = GenericTypePathCell::new();
///         CELL.get_or_insert::<Self>(||{
///             concat(&["Foo", "<", T::type_name(), ">"])
///         })
///     }
///     fn type_ident() -> &'static str { "Foo" }
///     fn module_path() -> Option<&'static str> { Some("my_crate::foo") }
/// }
/// ```
///
/// [`type_path`]: TypePath::type_path
/// [`type_name`]: TypePath::type_name
/// [`type_ident`]: TypePath::type_ident
/// [`module_path`]: TypePath::module_path
/// [`GenericTypePathCell`]: crate::impls::GenericTypePathCell
pub trait TypePath: 'static {
    /// Returns the fully qualified path with generics of the target type.
    ///
    /// This is the complete unique identifier of a type,
    /// and should **not** duplicated in different types.
    ///
    /// For `Option<Vec<usize>>`, this is `"core::option::Option<alloc::vec::Vec<usize>>"`.
    fn type_path() -> &'static str;

    /// Returns a short, pretty-print enabled path to the type.
    ///
    /// This name allows for duplication.
    ///
    /// Note that this is different from [`core::any::type_name`],
    /// the latter is more like [`TypePath::type_path`].
    ///
    /// For `Option<Vec<usize>>`, this is `"Option<Vec<usize>>"`.
    fn type_name() -> &'static str;

    /// Returns the short name of the type, without generics.
    ///
    /// For `Option<Vec<usize>>`, this is `"Option"`.
    fn type_ident() -> &'static str;

    /// Optional module path where the type is defined.
    ///
    /// Primitive built-in types may return `None`.
    ///
    /// For `Option<Vec<usize>>`, this is `Some("core::option")`.
    fn module_path() -> Option<&'static str> {
        None
    }
}

// -----------------------------------------------------------------------------
// DynamicTypePath

/// Provide dynamic dispatch for types that implement [`TypePath`].
///
/// Auto impl for all types that implemented [`TypePath`].
///
/// # Examples
///
/// ```
/// use voker_reflect::{info::DynamicTypePath, Reflect};
///
/// let x = String::from("");
/// assert_eq!(x.reflect_type_path(), "alloc::string::String");
///
/// // this is useful for reflect type.
/// let y: &dyn Reflect = &x;
/// assert_eq!(y.reflect_type_path(), "alloc::string::String");
/// ```
pub trait DynamicTypePath {
    /// Returns the fully qualified path with generics of the underlying type.
    ///
    /// See [`TypePath::type_path`].
    fn reflect_type_path(&self) -> &'static str;

    /// Returns a short, pretty-print enabled path to the type.
    ///
    /// See [`TypePath::type_name`].
    fn reflect_type_name(&self) -> &'static str;

    /// Returns the short name of the type, without generics.
    ///
    /// See [`TypePath::type_ident`].
    fn reflect_type_ident(&self) -> &'static str;

    /// Optional module path where the type is defined.
    ///
    /// See [`TypePath::module_path`].
    fn reflect_module_path(&self) -> Option<&'static str>;
}

impl<T: TypePath> DynamicTypePath for T {
    #[inline]
    fn reflect_type_path(&self) -> &'static str {
        Self::type_path()
    }

    #[inline]
    fn reflect_type_name(&self) -> &'static str {
        Self::type_name()
    }

    #[inline]
    fn reflect_type_ident(&self) -> &'static str {
        Self::type_ident()
    }

    #[inline]
    fn reflect_module_path(&self) -> Option<&'static str> {
        Self::module_path()
    }
}

// -----------------------------------------------------------------------------
// TypePathTable

/// Lightweight vtable providing dynamic access to [`TypePath`] APIs.
///
/// This struct stores function pointers to a type's `TypePath` implementations,
/// keeping initialization minimal for types that are rarely queried.
///
/// It provides an additional function [`crate_name`](TypePathTable::crate_name),
/// for parsing `crate_name` from `module_name`.
///
/// # Examples
///
/// ```
/// use voker_reflect::info::TypePathTable;
///
/// let x = TypePathTable::of::<String>();
/// assert_eq!(x.path(), "alloc::string::String");
/// assert_eq!(x.name(), "String");
/// assert_eq!(x.ident(), "String");
/// assert_eq!(x.module_path(), Some("alloc::string"));
/// assert_eq!(x.crate_name(), Some("alloc"));
/// ```
#[derive(Clone, Copy)]
pub struct TypePathTable {
    type_path: fn() -> &'static str,
    type_name: fn() -> &'static str,
    type_ident: fn() -> &'static str,
    module_path: fn() -> Option<&'static str>,
}

impl TypePathTable {
    /// Creates a new table from a type.
    #[inline]
    pub const fn of<T: TypePath + ?Sized>() -> Self {
        Self {
            type_path: T::type_path,
            type_name: T::type_name,
            type_ident: T::type_ident,
            module_path: T::module_path,
        }
    }

    /// See [`TypePath::type_path`]
    #[inline(always)]
    pub fn path(&self) -> &'static str {
        (self.type_path)()
    }

    /// See [`TypePath::type_name`]
    #[inline(always)]
    pub fn name(&self) -> &'static str {
        (self.type_name)()
    }

    /// See [`TypePath::type_ident`]
    #[inline(always)]
    pub fn ident(&self) -> &'static str {
        (self.type_ident)()
    }

    /// See [`TypePath::module_path`]
    #[inline(always)]
    pub fn module_path(&self) -> Option<&'static str> {
        (self.module_path)()
    }

    /// Parse `crate_name` from `module_path`.
    #[inline]
    pub fn crate_name(&self) -> Option<&'static str> {
        let s = (self.module_path)()?;
        let index = s.as_bytes().iter().position(|&item| item == b':');
        if let Some(index) = index {
            Some(&s[..index])
        } else {
            Some(s)
        }
    }
}

impl core::fmt::Debug for TypePathTable {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TypePathTable")
            .field("type_path", &self.path())
            .field("type_name", &self.name())
            .field("type_ident", &self.ident())
            .field("module_path", &self.module_path())
            .field("crate_name", &self.crate_name())
            .finish()
    }
}

// -----------------------------------------------------------------------------
// Type

/// The base representation of a Rust type.
///
/// Includes a [`TypeId`] and a [`TypePathTable`],
/// re-exported their functions.
///
/// # Examples
///
/// ```
/// # use core::any::TypeId;
/// use voker_reflect::info::Type;
///
/// let ty = Type::of::<String>();
///
/// assert!(ty.is::<String>());
/// assert_eq!(ty.path(), "alloc::string::String");
///
/// let type_id: TypeId = ty.id();
/// // ...
/// ```
#[derive(Copy, Clone)]
pub struct Type {
    type_path_table: TypePathTable,
    type_id: TypeId,
}

impl Type {
    /// Creates a new [`Type`] from a type that implements [`TypePath`].
    ///
    /// # Example
    ///
    /// ```
    /// # use voker_reflect::info::Type;
    /// let ty = Type::of::<String>();
    /// ```
    #[inline]
    pub const fn of<T: TypePath + ?Sized>() -> Self {
        Self {
            type_path_table: TypePathTable::of::<T>(),
            type_id: TypeId::of::<T>(),
        }
    }

    /// Returns the [`TypeId`] of the type.
    ///
    /// # Example
    ///
    /// ```
    /// # use core::any::TypeId;
    /// # use voker_reflect::info::Type;
    /// let ty = Type::of::<String>();
    /// assert_eq!(ty.id(), TypeId::of::<String>());
    /// ```
    #[inline(always)]
    pub const fn id(&self) -> TypeId {
        self.type_id
    }

    /// Check if the given type matches this one.
    ///
    /// This only compares the [`TypeId`] of the types.
    ///
    /// # Example
    ///
    /// ```
    /// # use voker_reflect::info::Type;
    /// let ty = Type::of::<String>();
    /// assert!(ty.is::<String>());
    /// ```
    #[inline(always)]
    pub fn is<T: Any>(&self) -> bool {
        TypeId::of::<T>() == self.type_id
    }

    /// Returns the [`TypePathTable`] of the type.
    ///
    /// It is usually recommended to directly use the re-export methos by [`Type`].
    /// Unless it is necessary to copy the `TypePathTable`.
    #[inline(always)]
    pub const fn path_table(&self) -> TypePathTable {
        self.type_path_table
    }

    /// See [`TypePath::type_path`].
    #[inline]
    pub fn path(&self) -> &'static str {
        self.type_path_table.path()
    }

    /// See [`TypePath::type_name`].
    #[inline]
    pub fn name(&self) -> &'static str {
        self.type_path_table.name()
    }

    /// See [`TypePath::type_ident`].
    #[inline]
    pub fn ident(&self) -> &'static str {
        self.type_path_table.ident()
    }

    /// See [`TypePath::module_path`].
    #[inline]
    pub fn module_path(&self) -> Option<&'static str> {
        self.type_path_table.module_path()
    }

    /// Parse `crate_name` from `module_path`.
    #[inline]
    pub fn crate_name(&self) -> Option<&'static str> {
        self.type_path_table.crate_name()
    }
}

/// This implementation purely relies on the [`TypeId`] of the type,
impl PartialEq for Type {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id
    }
}

impl Eq for Type {}

/// This implementation purely relies on the [`TypeId`] of the type,
impl core::hash::Hash for Type {
    #[inline]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.type_id.hash(state);
    }
}

/// This implementation will only output the [`TypePath`] of the type.
impl core::fmt::Debug for Type {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.path())
    }
}

// -----------------------------------------------------------------------------
// Auxiliary macro

macro_rules! impl_type_fn {
    ($field:ident) => {
        /// Returns the underlying `Type`.
        #[inline(always)]
        pub const fn ty(&self) -> &$crate::info::Type {
            &self.$field
        }
        $crate::info::impl_type_fn!();
    };
    ($self:ident => $expr:expr) => {
        /// Returns the underlying `Type`.
        #[inline(never)]
        pub const fn ty($self: &Self) -> &$crate::info::Type {
            $expr
        }
        $crate::info::impl_type_fn!();
    };
    () => {
        /// Returns the `TypePathTable`.
        #[inline]
        pub const fn type_path_table(&self) -> $crate::info::TypePathTable {
            self.ty().path_table()
        }

        /// Returns the `TypeId`.
        #[inline]
        pub const fn type_id(&self) -> ::core::any::TypeId {
            self.ty().id()
        }

        /// Check if the given type matches this one.
        #[inline]
        pub fn type_is<T: ::core::any::Any>(&self) -> bool {
            self.ty().id() == ::core::any::TypeId::of::<T>()
        }

        /// Returns the type path.
        #[inline]
        pub fn type_path(&self) -> &'static str {
            self.ty().path()
        }

        /// Returns the type name.
        #[inline]
        pub fn type_name(&self) -> &'static str {
            self.ty().name()
        }

        /// Returns the type ident.
        #[inline]
        pub fn type_ident(&self) -> &'static str {
            self.ty().ident()
        }

        /// Returns the module path.
        #[inline]
        pub fn module_path(&self) -> Option<&'static str> {
            self.ty().module_path()
        }

        /// Returns the crate name.
        #[inline]
        pub fn crate_name(&self) -> Option<&'static str> {
            self.ty().crate_name()
        }
    };
}

pub(crate) use impl_type_fn;

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    #[test]
    fn utf8_crate_name() {
        let s = "你好::world";

        let f = |s: &'static str| {
            let index = s.as_bytes().iter().position(|&item| item == b':');
            if let Some(index) = index {
                Some(&s[..index])
            } else {
                Some(s)
            }
        };

        let hello = f(s);
        assert_eq!(hello, Some("你好"));
    }
}
