use alloc::boxed::Box;
use core::ops::Deref;

use crate::info::{ConstParamData, Type, TypePath, impl_type_fn};

// -----------------------------------------------------------------------------
// Type Generic Param

/// Information about generic type parameters.
///
/// # Examples
///
/// ```
/// use voker_reflect::{Reflect, info::Typed};
///
/// #[derive(Reflect)]
/// struct Foo<T = usize>(T);
///
/// let info = <Foo<i32>>::type_info().generics().get("T").unwrap();
/// assert!(!info.is_const());
///
/// let info = info.as_type().unwrap();
/// assert!(info.type_is::<i32>());
///
/// let default_type = info.default().unwrap();
/// assert!(default_type.is::<usize>());
/// ```
#[derive(Clone, Debug)]
pub struct TypeParamInfo {
    ty: Type,
    name: &'static str,
    // reduce struct size
    default: Option<fn() -> Type>,
}

impl TypeParamInfo {
    impl_type_fn!(ty);

    /// Create a new [`TypeParamInfo`].
    #[inline]
    pub const fn new<T: TypePath + ?Sized>(name: &'static str) -> Self {
        Self {
            ty: Type::of::<T>(),
            name,
            default: None,
        }
    }

    /// Returns the generic parameter name.
    #[inline]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Set the default type for this parameter.
    #[inline]
    pub const fn with_default<T: TypePath + ?Sized>(mut self) -> Self {
        self.default = Some(Type::of::<T>);
        self
    }

    /// Returns the default type for this parameter, if present.
    #[inline]
    pub fn default(&self) -> Option<Type> {
        self.default.map(|f| f())
    }
}

// -----------------------------------------------------------------------------
// Const Generic Param

/// Information about a const generic parameter, size = 64.
///
/// When a type is instantiated, the value of the const generic is fixed,
/// so we provide value field, but there is no default value.
///
/// # Examples
///
/// ```
/// use voker_reflect::{Reflect, info::Typed};
///
/// #[derive(Reflect)]
/// struct Foo<const N: usize>;
///
/// let info = <Foo<10>>::type_info().generics().get("N").unwrap();
/// assert!(info.is_const());
///
/// let info = info.as_const().unwrap();
/// assert!(info.type_is::<usize>());
/// assert_eq!(info.value::<usize>(), Some(10usize));
/// ```
#[derive(Clone, Debug)]
pub struct ConstParamInfo {
    ty: Type,
    name: &'static str,
    // reduce struct size
    value: Box<ConstParamData>,
}

impl ConstParamInfo {
    impl_type_fn!(ty);

    /// Create a new [`ConstParamInfo`] with the value of const generic param.
    #[inline]
    pub fn new<T: TypePath + Into<ConstParamData>>(name: &'static str, value: T) -> Self {
        Self {
            ty: Type::of::<T>(),
            name,
            value: Box::new(value.into()),
        }
    }

    /// Returns the generic parameter name.
    #[inline]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the const value for this parameter, if the type is correct.
    #[inline]
    pub fn value<T: TryFrom<ConstParamData>>(&self) -> Option<T> {
        (*self.value).try_into().ok()
    }
}

// -----------------------------------------------------------------------------
// Single Generic

/// A single generic parameter (either a type or a const).
///
/// # Examples
///
/// ```
/// use voker_reflect::{Reflect, info::Typed};
///
/// #[derive(Reflect)]
/// struct Foo<T, const N: usize>([T; N]);
///
/// let info = <Foo<i32, 5>>::type_info().generics();
///
/// let type_info = info.get("T").unwrap();
/// assert!(type_info.type_is::<i32>());
/// assert!(!type_info.is_const());
/// let const_info = info.get("N").unwrap();
/// assert!(const_info.type_is::<usize>());
/// assert!(const_info.is_const());
/// ```
#[derive(Clone, Debug)]
pub enum GenericInfo {
    Type(TypeParamInfo),
    Const(ConstParamInfo),
}

impl From<TypeParamInfo> for GenericInfo {
    #[inline(always)]
    fn from(value: TypeParamInfo) -> Self {
        Self::Type(value)
    }
}

impl From<ConstParamInfo> for GenericInfo {
    #[inline(always)]
    fn from(value: ConstParamInfo) -> Self {
        Self::Const(value)
    }
}

impl GenericInfo {
    impl_type_fn!(self => match self {
        Self::Type(info) => info.ty(),
        Self::Const(info) => info.ty(),
    });

    #[inline]
    pub const fn as_type(&self) -> Option<&TypeParamInfo> {
        match self {
            GenericInfo::Type(info) => Some(info),
            _ => None,
        }
    }

    #[inline]
    pub const fn as_const(&self) -> Option<&ConstParamInfo> {
        match self {
            GenericInfo::Const(info) => Some(info),
            _ => None,
        }
    }

    /// Returns the parameter name.
    #[inline]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Type(info) => info.name(),
            Self::Const(info) => info.name(),
        }
    }

    /// Returns `true` if this parameter is a type parameter.
    #[inline]
    pub const fn is_type(&self) -> bool {
        matches!(self, Self::Type(_))
    }

    /// Returns `true` if this parameter is a const parameter.
    #[inline]
    pub const fn is_const(&self) -> bool {
        matches!(self, Self::Const(_))
    }
}

// -----------------------------------------------------------------------------
// Generics

/// A container for a list of generic parameters.
///
/// This is automatically generated via the [`#[derive(Reflect)]`](crate::derive::Reflect),
/// and stored on the [`TypeInfo`] returned by [`Typed::type_info`] for types that have generics.
///
/// It supports both type parameters and const parameters so long as they implement [`TypePath`].
///
/// If the type has no generics, this will be empty.
///
/// # Examples
///
/// ## GenericInfo
///
/// A enum of `TypeParamInfo` and `ConstParamInfo`, see [`GenericInfo`] .
///
/// ```
/// use voker_reflect::{Reflect, info::Typed};
///
/// #[derive(Reflect)]
/// struct Foo<T, const N: usize>([T; N]);
///
/// let info = <Foo<i32, 5>>::type_info().generics();
///
/// let type_info = &info[0];
/// assert!(!type_info.is_const());
/// assert!(type_info.type_is::<i32>());
/// assert_eq!(type_info.name(), "T");
///
/// let const_info = &info[1];
/// assert!(const_info.is_const());
/// assert!(const_info.type_is::<usize>());
/// assert_eq!(const_info.name(), "N");
/// ```
///
/// ## TypeParamInfo
///
/// See [`TypeParamInfo`] .
///
/// ```
/// use voker_reflect::{Reflect, info::Typed};
///
/// #[derive(Reflect)]
/// struct Foo<T = usize>(T);
///
/// let info = <Foo<i32>>::type_info().generics().get("T").unwrap();
/// assert!(!info.is_const());
///
/// let info = info.as_type().unwrap();
/// assert!(info.type_is::<i32>());
///
/// let default_type = info.default().unwrap();
/// assert!(default_type.is::<usize>());
/// ```
///
/// ## ConstParamInfo
///
/// See [`ConstParamInfo`] .
///
/// ```
/// use voker_reflect::{Reflect, info::Typed};
///
/// #[derive(Reflect)]
/// struct Foo<const N: usize>;
///
/// let info = <Foo<10>>::type_info().generics().get("N").unwrap();
/// assert!(info.is_const());
///
/// let info = info.as_const().unwrap();
/// assert!(info.type_is::<usize>());
/// assert_eq!(info.value::<usize>(), Some(10usize));
/// ```
///
/// [`TypeInfo`]: voker_reflect::info::TypeInfo
/// [`Typed::type_info`]: voker_reflect::info::Typed::type_info
#[derive(Clone, Default, Debug)]
pub struct Generics(Option<Box<[GenericInfo]>>);

impl Generics {
    /// Create a new, empty `Generics` container.
    #[inline(always)]
    pub const fn new() -> Self {
        // We use `Option` to enable compile time `new`.
        // The pointer cannot be null, which ensures that
        // the `Option` does not change the type size.
        Self(None)
    }

    /// Create a `Generics` from `GenericInfo`s.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::info::{Generics, GenericInfo, TypeParamInfo, ConstParamInfo};
    /// // The generics of `[i32; 5]` :
    /// let array_i32 = Generics::from([
    ///     GenericInfo::Type(TypeParamInfo::new::<i32>("T")),
    ///     GenericInfo::Const(ConstParamInfo::new::<usize>("N", 5)),
    /// ]);
    /// ```
    #[inline]
    pub fn from<const P: usize>(buf: [GenericInfo; P]) -> Self {
        Self(Some(Box::new(buf)))
    }

    /// Returns the `GenericInfo` for the parameter with the given `name`,
    /// if present.
    ///
    /// Complexity: O(n) in the number of parameters.
    ///
    /// ```
    /// use voker_reflect::{Reflect, info::Typed};
    ///
    /// #[derive(Reflect)]
    /// struct Foo<T = usize>(T);
    ///
    /// let generics = <Foo<i32>>::type_info().generics();
    /// let info = generics.get("T").unwrap();
    /// assert!(info.is_type());
    /// ```
    pub fn get(&self, name: &str) -> Option<&GenericInfo> {
        match &self.0 {
            Some(val) => val.iter().find(|info| info.name() == name),
            None => None,
        }
    }
}

impl Deref for Generics {
    type Target = [GenericInfo];
    #[inline]
    fn deref(&self) -> &Self::Target {
        match &self.0 {
            Some(v) => v,
            None => &[],
        }
    }
}

// -----------------------------------------------------------------------------
// Auxiliary macro

/// Implement `with_generics` and `generics`.
macro_rules! impl_generic_fn {
    ($field:ident) => {
        $crate::info::generics::impl_generic_fn!(self => &self.$field);

        /// Replace its own generic information
        #[inline]
        pub fn with_generics(
            mut self,
            generics: $crate::info::Generics
        ) -> Self {
            self.$field = generics;
            self
        }
    };
    ($self:ident => $expr:expr) => {
        /// Returns generic metadata.
        ///
        /// See [`Generics`](crate::info::Generics).
        #[inline]
        pub const fn generics($self: &Self) -> &$crate::info::Generics {
            $expr
        }
    };
}

pub(super) use impl_generic_fn;
