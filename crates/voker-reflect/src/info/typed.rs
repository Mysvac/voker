use crate::info::{TypeInfo, TypePath};

// -----------------------------------------------------------------------------
// Typed

/// A static accessor to compile-time type information.
///
/// Automatically implemented by [`#[derive(Reflect)]`](crate::derive::Reflect),
/// allowing access to type information without an instance of the type.
///
/// # Examples
///
/// ```
/// use voker_reflect::{Reflect, info::{Typed, TypeInfo}};
///
/// #[derive(Reflect)]
/// struct A{ /* ... */ }
///
/// let info: &'static TypeInfo = <A as Typed>::type_info();
/// ```
///
/// # Manually Impl
///
/// It is not recommended to implement manually. But we provided [`NonGenericTypeInfoCell`]
/// and [`GenericTypeInfoCell`] to simplify it, if it's necessary.
///
/// For non-generic type:
///
/// ```
/// use voker_reflect::{
///     Reflect,
///     info::{Typed, TypeInfo, StructInfo, NamedField},
///     impls::NonGenericTypeInfoCell
/// };
///
/// #[derive(Reflect)]
/// #[reflect(Typed = false)]
/// struct NonGenericStruct {
///   foo: usize,
///   bar: (f32, f32)
/// }
///
/// impl Typed for NonGenericStruct {
///     fn type_info() -> &'static TypeInfo {
///         static CELL: NonGenericTypeInfoCell = NonGenericTypeInfoCell::new();
///         CELL.get_or_init(|| TypeInfo::Struct(
///             StructInfo::new::<Self>(&[
///                 NamedField::new::<usize>("foo"),
///                 NamedField::new::<(f32, f32)>("bar"),
///             ])
///         ))
///     }
/// }
/// ```
///
/// For generic types:
///
/// ```
/// use voker_reflect::{
///     Reflect,
///     info::{Typed, TypeInfo, TupleStructInfo, UnnamedField},
///     impls::GenericTypeInfoCell
/// };
///
/// #[derive(Reflect)]
/// #[reflect(Typed = false)]
/// struct GenericTupleStruct<T>(T);
///
/// impl<T: Typed + Reflect> Typed for GenericTupleStruct<T> {
///     fn type_info() -> &'static TypeInfo {
///         static CELL: GenericTypeInfoCell = GenericTypeInfoCell::new();
///         CELL.get_or_insert::<Self>(|| TypeInfo::TupleStruct(
///             TupleStructInfo::new::<Self>(&[
///                 UnnamedField::new::<T>(0), // `0` is field index
///             ])
///         ))
///     }
/// }
/// ```
///
/// [`NonGenericTypeInfoCell`]: crate::impls::NonGenericTypeInfoCell
/// [`GenericTypeInfoCell`]: crate::impls::GenericTypeInfoCell
pub trait Typed: TypePath {
    /// A static accessor to compile-time type information.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::{Reflect, info::Typed};
    /// #[derive(Reflect)]
    /// struct A{ /* ... */ }
    /// let info = <A as Typed>::type_info();
    /// ```
    ///
    /// Note: Use [`DynamicTyped`] for dynamic dispatch.
    fn type_info() -> &'static TypeInfo;
}

// -----------------------------------------------------------------------------
// DynamicTyped

/// Provide dynamic dispatch for types that implement [`Typed`].
///
/// Auto impl for all types that implemented [`Typed`].
pub trait DynamicTyped {
    /// Provide dynamic dispatch for types that implement [`Typed`].
    ///
    /// When you hold a `dyn Reflect` object,
    /// can to use this method to get type information.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, info::DynamicTyped};
    /// #[derive(Reflect)]
    /// struct A(u64);
    ///
    /// let a = Box::new(A(1)) as Box<dyn Reflect>;
    /// let info = a.reflect_type_info();
    /// ```
    fn reflect_type_info(&self) -> &'static TypeInfo;
}

impl<T: Typed> DynamicTyped for T {
    #[inline]
    fn reflect_type_info(&self) -> &'static TypeInfo {
        Self::type_info()
    }
}
