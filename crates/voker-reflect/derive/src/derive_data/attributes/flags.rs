use proc_macro2::Span;

/// A struct used to control whether a trait needs to be implemented.
pub(crate) struct TraitImplSwitches {
    /// Default is `true`, use `#[reflect(TypePath = false)]`  to disable it.
    /// Then Users can(must) impl it in a more customized way.
    pub(crate) impl_type_path: bool,
    /// Default is `true`, use `#[reflect(Typed = false)]`  to disable it.
    /// Then Users can(must) impl it in a more customized way.
    pub(crate) impl_typed: bool,
    /// Default is `true`, use `#[reflect(Reflect = false)]`  to disable it.
    /// Then Users can(must) impl it in a more customized way.
    pub(crate) impl_reflect: bool,
    /// Default is `true`, use `#[reflect(GetTypeMeta = false)]`  to disable it.
    /// Then Users can(must) impl it in a more customized way.
    pub(crate) impl_get_type_meta: bool,
    /// Default is `true`, use `#[reflect(FromReflect = false)]`  to disable it.
    /// Then Users can(must) impl it in a more customized way.
    pub(crate) impl_from_reflect: bool,
    /// Default is `true`, use `#[reflect(Struct = false)]`  to disable it.
    /// Even if it is true, it only takes effect when the type is correct.
    pub(crate) impl_struct: bool,
    /// Default is `true`, use `#[reflect(TupleStruct = false)]`  to disable it.
    /// Even if it is true, it only takes effect when the type is correct.
    pub(crate) impl_tuple_struct: bool,
    /// Default is `true`, use `#[reflect(Tuple = false)]`  to disable it.
    /// Even if it is true, it only takes effect when the type is correct.
    pub(crate) impl_tuple: bool,
    /// Default is `true`, use `#[reflect(Enum = false)]`  to disable it.
    /// Even if it is true, it only takes effect when the type is correct.
    pub(crate) impl_enum: bool,
}

impl Default for TraitImplSwitches {
    #[inline]
    fn default() -> Self {
        Self {
            impl_type_path: true,
            impl_typed: true,
            impl_reflect: true,
            impl_get_type_meta: true,
            impl_from_reflect: true,
            impl_struct: true,
            impl_tuple_struct: true,
            impl_tuple: true,
            impl_enum: true,
        }
    }
}

/// A struct used to record whether the specified trait is available.
#[derive(Default)]
pub(crate) struct TraitAvailableFlags {
    pub default: Option<Span>,
    pub clone: Option<Span>,
    pub debug: Option<Span>,
    pub hash: Option<Span>,
    pub eq: Option<Span>,
    pub cmp: Option<Span>,
    pub serialize: Option<Span>,
    pub deserialize: Option<Span>,
}
