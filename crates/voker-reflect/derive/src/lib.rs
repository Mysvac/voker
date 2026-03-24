//! See following macros:
//!
//! - [`Reflect`]
//! - [`TypePath`]
//! - [`impl_reflect`]
//! - [`impl_reflect_opaque`]
//! - [`impl_type_path`]
//! - [`impl_auto_register`]
//! - [`reflect_trait`]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(clippy::std_instead_of_core, reason = "proc-macro lib")]
#![allow(clippy::std_instead_of_alloc, reason = "proc-macro lib")]

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

static REFLECT_ATTRIBUTE: &str = "reflect";

// -----------------------------------------------------------------------------
// Modules

mod derive_data;
mod impls;
mod path;
mod utils;

// -----------------------------------------------------------------------------
// Macros

/// # Full Reflection Derivation
///
/// `#[derive(Reflect)]` automatically implements the following traits:
///
/// - `TypePath`
/// - `Typed`
/// - `Reflect`
/// - `GetTypeMeta`
/// - `FromReflect`
/// - `Struct` (for `struct T { ... }`)
/// - `TupleStruct` (for `struct T(...);`)
/// - `Enum` (for `enum T { ... }`)
///
/// Note: Unit structs (`struct T;`) are treated as `Opaque` rather than as composite types like `Struct`.
///
/// ## Implementation Control
///
/// ### Disabling Default Implementations
///
/// You can disable specific implementations using attributes; in such cases, you must provide them manually.
///
/// ```rust, ignore
/// #[derive(Reflect)]
/// #[reflect(TypePath = false, Typed = false)]
/// struct Foo { /* ... */ }
/// ```
///
/// All the toggles mentioned above can be disabled; explicitly enabling them is redundant as it's the default behavior.
///
/// These attributes can only be applied at the type level (not on fields).
///
/// ### Custom Type Path
///
/// Since `TypePath` often requires customization, an attribute is provided to override the default path:
///
/// ```rust, ignore
/// #[derive(Reflect)]
/// #[reflect(type_path = "you::me::Foo")]
/// struct Foo { /* ... */ }
/// ```
///
/// This path does not need to include generics (they will be automatically appended).
///
/// This attribute can only be applied at the type level.
///
/// ### Opaque Types
///
/// Unit structs like `struct A;` are treated as `Opaque`. They contain no internal data,
/// allowing the macro to automatically generate methods like `reflect_clone`, `reflect_eq`, etc.
///
/// ```rust, ignore
/// #[derive(Reflect)]
/// struct MyFlag;
/// ```
///
/// The `Opaque` attribute forces a type to be treated as `Opaque` instead of `Struct`, `Enum`, or `TupleStruct`.
///
/// When you mark a type as `Opaque`, the macro will not inspect its internal fields; consequently, methods such
/// as `reflect_clone` or `reflect_hash` that depend on field content cannot be generated automatically. Therefore,
/// `Opaque` types must implement `Clone` and be marked with the `clone` flag when applicable:
///
/// ```rust, ignore
/// #[derive(Reflect)]
/// #[reflect(Opaque, clone)]
/// struct Foo { /* ... */ }
///
/// impl Clone for Foo {  /* ... */ }
/// ```
///
/// This attribute can only be applied at the type level.
///
/// ## Optimization with Standard Traits
///
/// If a type implements standard traits like `Hash` or `Clone`, the reflection implementations can be simplified
/// (often resulting in significant performance improvements). The macro cannot detect this automatically, so it does
/// not assume their availability by default. Use attributes to declare available traits so the macro can optimize
/// accordingly.
///
/// As noted, `Opaque` types require `Clone` support, so they must implement it and be marked with the `clone` flag.
///
/// ```rust, ignore
/// #[derive(Reflect)]
/// #[reflect(Opaque, clone, hash)]
/// struct Foo { /* ... */ }
/// // impl Clone, Hash ...
/// ```
///
/// Available flags:
///
/// - `clone`: Standard `Clone`
/// - `hash`: Standard `Hash`
/// - `eq`: Standard `PartialEq`
/// - `cmp`: Standard `PartialOrd`
/// - `default`: Standard `Default`
/// - `serialize`: `serde::Serialize`
/// - `deserialize`: `serde::Deserialize`
///
/// Two convenience bundles enable multiple flags simultaneously:
///
/// - `serde`: `serialize` + `deserialize` + `auto_register`
/// - `full`: All seven traits listed above + `auto_register`
///
/// These attributes can only be applied at the type level.
///
/// ## Auto Registration
///
/// Unlike Bevy, automatic type registration is disabled by default (even when the `auto_register` feature is enabled).
/// You must explicitly enable it using the `auto_register` attribute.
///
/// ### Example
///
/// ```rust, ignore
/// #[derive(Reflect)]
/// #[reflect(auto_register)]
/// struct A { /* ... */ }
/// ```
///
/// Note: This macro has no effect on generic types, as we cannot determine which concrete types will be instantiated.
///
/// This attribute is a no-op when the `auto_register` feature is disabled.
///
/// This attribute can only be applied at the type level.
///
/// ## Custom GetTypeMeta
///
/// By default, a type's `get_type_meta` includes at least `ReflectFromPtr`. The following type traits may also be
/// included based on conditions:
///
/// - `ReflectFromReflect`: If the default `FromReflect` implementation is enabled (not disabled with
///   `#[reflect(FromReflect = false)]`).
/// - `ReflectDefault`: If `Default` is marked as available via `#[reflect(default)]`.
/// - `ReflectSerialize`: If `serde::Serialize` is marked as available via `#[reflect(serialize)]`.
/// - `ReflectDeserialize`: If `serde::Deserialize` is marked as available via `#[reflect(deserialize)]`.
///
/// You can also manually add type traits using `#[reflect(type_trait = (...))]`. These will be automatically
/// inserted into `get_type_meta`.
///
/// ### Example
///
/// ```rust, ignore
/// #[derive(Reflect)]
/// #[reflect(type_trait = ReflectPrint)]
/// struct A;
///
/// #[derive(Reflect)]
/// #[reflect(type_trait = (ReflectDebug, ReflectClone, ReflectDisplay))]
/// struct A;
/// ```
///
/// This attribute can only be applied at the type level.
///
/// ## Documentation Reflection
///
/// Enable the `reflect_docs` feature to include documentation in type information.
///
/// By default, the macro collects `#[doc = "..."]` attributes (including `/// ...` comments).
///
/// To disable documentation collection for a specific type, use `#[reflect(doc = false)]`:
///
/// ```rust, ignore
/// /// Example doc comments
/// #[derive(Reflect)]
/// #[reflect(doc = false)]
/// struct A;
/// ```
///
/// To provide custom documentation instead of collecting `#[doc = "..."]` attributes, use one or more `#[reflect(doc = "...")]` attributes:
///
/// ```rust, ignore
/// /// Default comments
/// /// ...
/// #[derive(Reflect)]
/// #[reflect(doc = "Custom comments, line 1.")]
/// #[reflect(doc = "Custom comments, line 2.")]
/// struct A;
/// ```
///
/// When the macro detects `#[reflect(doc = "...")]`, it stops collecting standard `#[doc = "..."]` documentation.
///
/// This attribute is a no-op when the `reflect_docs` feature is disabled.
///
/// This attribute can be applied at the type, field, and enum variant levels.
///
/// ## Custom Attributes
///
/// We support adding custom attributes to types, similar to C# attributes.
///
/// The syntax is `#[reflect(@Expr)]`. For example:
///
/// ```rust, ignore
/// #[derive(Reflect)]
/// #[reflect(@0.1_f32)]
/// struct A {
///     #[reflect(@false, @"data")]
///     data: Vec<u8>,
/// }
/// ```
///
/// These attributes can be retrieved from the type's `TypeInfo`.
///
/// Any type implementing `Reflect` can be used as an attribute. However,
/// note that attributes are stored by type, and multiple attributes of the same type cannot coexist
/// (the last one will overwrite previous ones).
///
/// ```rust, ignore
/// #[derive(Reflect)]
/// #[reflect(@1_i32, @2_i32)]
/// struct A;
/// // Only `2_i32` will be stored, overwriting `1_i32`.
/// ```
///
/// This attribute can be applied at the type, field, and enum variant levels.
///
/// ## skip_serde
///
/// There is a special attribute called `skip_serde`, which can only be used on fields.
///
/// This attribute skips the field during serialization and uses the provided `ReflectDefault` during deserialization.
///
/// ```rust, ignore
/// #[derive(Reflect)]
/// struct A<T> {
///     text: String
///     #[reflect(skip_serde)]
///     _marker: PhantomData<T>,
/// }
/// ```
///
/// Important: This only takes effect with the default serialization provided by the reflection system.
/// If the type is annotated with `reflect(serde)` and supports serialization via the serde library,
/// this field attribute will not have any effect.
#[proc_macro_derive(Reflect, attributes(reflect))]
pub fn derive_full_reflect(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    impls::match_reflect_impls(ast, ImplSourceKind::DeriveLocalType)
}

/// # Derive TypePath Trait
///
/// This macro only implements `TypePath` trait,
///
/// The usage is similar to [`derive Reflect`](derive_full_reflect).
///
/// ## Example
///
/// ```rust, ignore
/// // default implementation
/// #[derive(TypePath)]
/// struct A;
///
/// // custom implementation
/// #[derive(TypePath)]
/// #[reflect(type_path = "crate_name::foo::B")]
/// struct B;
///
/// // support generics
/// #[derive(TypePath)]
/// #[reflect(type_path = "crate_name::foo::C")]
/// struct C<T>(T);
/// ```
#[proc_macro_derive(TypePath, attributes(reflect))]
pub fn derive_type_path(input: TokenStream) -> TokenStream {
    use crate::derive_data::{ReflectMeta, TypeAttributes, TypeParser};

    let ast: DeriveInput = parse_macro_input!(input as DeriveInput);

    let type_attributes = match TypeAttributes::parse_attrs(&ast.attrs) {
        Ok(v) => v,
        Err(err) => return err.into_compile_error().into(),
    };

    let type_parser =
        TypeParser::new_local(&ast.ident, type_attributes.type_path.clone(), &ast.generics);

    let meta = ReflectMeta::new(type_attributes, type_parser);
    impls::impl_trait_type_path(&meta).into()
}

/// Implements reflection for foreign types.
///
/// It requires full type information and access to fields.
/// Because of the orphan rule, this is typically used inside the reflection crate itself.
///
/// The usage is similar to [`derive Reflect`](derive_full_reflect).
///
/// ## Example
///
/// ```rust, ignore
/// impl_reflect! {
///     #[reflect(type_path = "core::option:Option")]
///     enum Option<T> {
///         Some(T),
///         None,
///     }
/// }
/// ```
///
/// See [`derive Reflect`](derive_full_reflect) for more details.
#[proc_macro]
pub fn impl_reflect(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    impls::match_reflect_impls(ast, ImplSourceKind::ImplForeignType)
}

/// How the macro was invoked.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum ImplSourceKind {
    /// Using `impl_full_reflect!`.
    ImplForeignType,
    /// Using `#[derive(...)]`.
    DeriveLocalType,
}

/// Implements reflection for `Opaque` types.
///
/// Syntax: `(in module_path as alias_name) ident (..attrs..)`.
///
/// ## Example
///
/// ```rust, ignore
/// impl_reflect_opaque!(u64 (full));
/// impl_reflect_opaque!(::utils::One<T: Clone> (clone));
/// impl_reflect_opaque!(::alloc::string::String (clone, debug, docs = "hello"));
/// impl_reflect_opaque!((in core::time) Instant (clone));
/// impl_reflect_opaque!((in core::time as Ins) Instant (clone));
/// ```
///
/// This macro always implies `Opaque`, so `clone` is required.
///
/// See available attributes in [`derive Reflect`](derive_full_reflect) .
#[proc_macro]
pub fn impl_reflect_opaque(input: TokenStream) -> TokenStream {
    use crate::derive_data::{ReflectMeta, ReflectOpaqueParser, TypeParser};

    let ReflectOpaqueParser {
        attrs,
        custom_path,
        type_ident,
        type_path,
        generics,
    } = parse_macro_input!(input with ReflectOpaqueParser::parse);

    let parser = TypeParser::new_foreign(&type_ident, &type_path, custom_path, &generics);

    let meta = ReflectMeta::new(attrs, parser);

    let reflect_impls = impls::impl_opaque(&meta);

    quote! {
        const _: () = {
            #reflect_impls
        };
    }
    .into()
}

/// A macro that implements `TypePath` for foreign type.
///
/// Syntax: `(in module_path as alias_name) ident`.
///
/// Paths starting with `::` cannot be used for primitive types.
/// The specified path must resolve to the target type and be accessible from the crate where the macro is invoked.
///
/// ## Example
///
/// ```ignore
/// // impl for primitive type.
/// impl_type_path!(u64);
///
/// // Implement for specified type.
/// impl_type_path!(::alloc::string::String);
/// // The prefix `::` will be removed by the macro, but it's required.
/// // This indicates that this is a complete path.
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
/// See: [`derive Reflect`](derive_full_reflect)
#[proc_macro]
pub fn impl_type_path(input: TokenStream) -> TokenStream {
    use crate::derive_data::{ReflectMeta, ReflectTypePathParser, TypeAttributes, TypeParser};

    let ReflectTypePathParser {
        custom_path,
        type_ident,
        type_path,
        generics,
    } = parse_macro_input!(input with ReflectTypePathParser::parse);

    let parser = TypeParser::new_foreign(&type_ident, &type_path, custom_path, &generics);

    let meta = ReflectMeta::new(TypeAttributes::default(), parser);

    let type_path_impls = impls::impl_trait_type_path(&meta);

    quote! {
        const _: () = {
            #type_path_impls
        };
    }
    .into()
}

/// Add the type to the automatic registry.
///
/// If the feature is not enabled, this macro will not do anything.
///
/// The type must be concrete (no uncertain generic parameters).
///
/// ## Example
///
/// ```ignore
/// impl_auto_register!(foo::Foo);
/// impl_auto_register!(Vec<u32>); // Ok
/// impl_auto_register!(Vec<T: Clone>); // Error
/// ```
///
/// This does not conflict with the `reflect(auto_register)` attribute.
///
/// See: [`derive Reflect`](derive_full_reflect)
#[proc_macro]
pub fn impl_auto_register(input: TokenStream) -> TokenStream {
    if ::core::cfg!(feature = "auto_register") {
        let type_path = syn::parse_macro_input!(input as syn::Type);

        let voker_reflect_path = path::voker_reflect();
        let auto_register_ =
            path::auto_register_(&voker_reflect_path, ::proc_macro2::Span::call_site());

        TokenStream::from(quote! {
            const _: () = {
                #auto_register_::inventory::submit!{
                    #auto_register_::__AutoRegisterFunc(
                        <#type_path as #auto_register_::__RegisterType>::__register
                    )
                }
            };
        })
    } else {
        utils::empty().into()
    }
}

/// Impl `TypeTrait` for specific trait with a new struct.
///
/// This macro will generate a `{trait_name}FromReflect` struct, which implements `TypeTrait` and `TypePath`.
///
/// For example, for `Display`, this will generate `DisplayReflect`.
///
/// It only contains three methods internally:
/// - `from_ref`: cast `&dyn Reflect` to `&dyn {trait_name}`
/// - `from_mut`: cast `&mut dyn Reflect` to `&mut dyn {trait_name}`
/// - `from_boxed`: cast `Box<dyn Reflect>` to `Box<dyn {trait_name}>`
///
/// ## Example
///
/// ```ignore
/// #[reflect_trait]
/// pub trait MyDebug {
///     fn debug(&self);
/// }
///
/// impl MyDebug for String { /* ... */ }
///
/// let reg = TypeRegistry::new()
///     .register::<String>()
///     .register_type_trait::<String, MyDebugFromReflect>();
///
/// let x: Box<dyn Reflect> = Box::new(String::from("123"));
///
/// let my_debug_from = reg.get_type_trait::<MyDebugFromReflect>::((*x).type_id()).unwrap();
/// let x: Box<dyn MyDebug> = my_debug_from.from_boxed(x);
/// x.debug();
/// ```
#[proc_macro_attribute]
pub fn reflect_trait(_args: TokenStream, input: TokenStream) -> TokenStream {
    impls::impl_reflect_trait(input)
}
