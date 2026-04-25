//! Reflection Macros
#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(clippy::std_instead_of_core, reason = "proc-macro lib")]
#![allow(clippy::std_instead_of_alloc, reason = "proc-macro lib")]

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

static REFLECT_ATTRIBUTE: &str = "reflect";
static TYPE_DATA_ATTRIBUTE: &str = "type_data";
static TYPE_PATH_ATTRIBUTE: &str = "type_path";

// -----------------------------------------------------------------------------
// Modules

mod derive_data;
mod impls;
mod path;
mod string_expr;

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
/// #[type_path = "you::me::Foo"]
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
/// `Opaque` types must declare either `Clone` or `NotCloneable`.
///
/// ```rust, ignore
/// #[derive(Reflect)]
/// #[reflect(Opaque, Clone)]
/// struct Foo { /* ... */ }
///
/// impl Clone for Foo {  /* ... */ }
///
/// #[derive(Reflect)]
/// #[reflect(Opaque, NotCloneable)]
/// struct Bar { /* ... */ }
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
/// As noted, `Opaque` types require either `Clone` or `NotCloneable` to be explicitly marked.
///
/// ```rust, ignore
/// #[derive(Reflect)]
/// #[reflect(Opaque, Clone, Hash)]
/// struct Foo { /* ... */ }
/// // impl Clone, Hash ...
/// ```
///
/// Available flags:
///
/// - `Clone`: Standard `Clone`
/// - `NotCloneable`: Force `reflect_clone` to return `ReflectCloneError::NotSupport`
/// - `Hash`: Standard `Hash`
/// - `PartialEq`: Standard `PartialEq`
/// - `PartialOrd`: Standard `PartialOrd`
/// - `Default`: Standard `Default`
/// - `Serialize`: `serde::Serialize`
/// - `Deserialize`: `serde::Deserialize`
///
/// These attributes can only be applied at the type level.
///
/// ## Reflection-Based Type Conversion
///
/// If a type implements `Into<T>` or `From<T>` for some other reflected type `T`, you can declare
/// these conversions so they are accessible through the reflection system at runtime.
///
/// ```rust, ignore
/// #[derive(Reflect)]
/// #[reflect(Into<f64>, From<i32>)]
/// struct MyNum(f32);
///
/// impl Into<f64> for MyNum { /* ... */ }
/// impl From<i32> for MyNum { /* ... */ }
/// ```
///
/// This inserts a [`ReflectConvert`] entry into the type's `TypeMeta`, which can later be retrieved
/// from a [`TypeRegistry`] to perform dynamic conversions between reflected types:
///
/// These attributes can only be applied at the type level.
///
/// ## Custom GetTypeMeta
///
/// By default, The following type traits may be included based on conditions:
///
/// - `ReflectFromReflect`: If the default `FromReflect` implementation is enabled (not disabled with
///   `#[reflect(FromReflect = false)]`).
/// - `ReflectDefault`: If `Default` is marked as available via `#[reflect(Default)]`.
/// - `ReflectSerialize`: If `serde::Serialize` is marked as available via `#[reflect(Serialize)]`.
/// - `ReflectDeserialize`: If `serde::Deserialize` is marked as available via `#[reflect(Deserialize)]`.
///
/// You can also manually add type traits using `#[type_data(...)]`. These will be automatically
/// inserted into `get_type_meta`.
///
/// ### Example
///
/// ```rust, ignore
/// #[derive(Reflect, Component)]
/// #[type_data(ReflectComponent)]
/// struct A;
///
/// #[derive(Reflect)]
/// #[type_data(ReflectDebug, ReflectClone, ReflectDisplay)]
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
/// ## auto_register
///
/// When a non-generic type (lifetimes are allowed, but type and const
/// parameters are not) is derived with `Reflect`, and `GetTypeMeta`
/// generation is not explicitly disabled, the macro also generates
/// auto-registration code for that type.
///
/// Auto-registration relies on static initialization, which requires a
/// concrete type. Because of that, generic parameters cannot be collected
/// automatically. Static registration also requires `TypeMeta`, so this
/// behavior is tied to `GetTypeMeta` generation.
///
/// For generic types, use [`impl_auto_register`] to register concrete
/// instantiations. For remote types where [`impl_auto_register`] is not
/// applicable, consider using [`auto_register`] instead.
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
/// If the type is annotated with `reflect(Serialize, Deserialize)` and supports serialization via the serde library,
/// this field attribute will not have any effect.
///
/// ## ignore
///
/// There is a special attribute called `ignore`, which can only be used on fields.
///
/// ```rust, ignore
/// #[derive(Reflect)]
/// struct A<T> {
///     text: String
///     #[reflect(ignore)]
///     _marker: PhantomData<T>,
/// }
/// ```
///
/// Unlike `skip_serde`, this attribute causes the field to be completely excluded from reflection.
/// It cannot be accessed through any reflection APIs, and the reflected `field_len` will not count this field.
///
/// This makes `reflect_clone` and `from_reflect` difficult to implement. Therefore, alongside `ignore`,
/// there are companion attributes `clone` and `default`, which can only be used on `ignore`d fields:
///
/// ```rust, ignore
/// #[derive(Reflect)]
/// struct A<T> {
///     text: String
///     #[reflect(ignore, clone, default)]
///     _marker: PhantomData<T>,
/// }
/// ```
///
/// When the complete type does not directly implement `Clone`, `reflect_clone` clones each field individually.
/// The `clone` attribute ensures that such ignored fields can be cloned properly at that time.
///
/// When `Clone` fails and the complete type does not implement `Default`, `from_reflect` initializes each field individually.
/// The `default` attribute ensures that such ignored fields can be constructed properly at that time.
#[proc_macro_derive(Reflect, attributes(reflect, type_data, type_path))]
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
/// #[type_path = "crate_name::foo::B"]
/// struct B;
///
/// // support generics
/// #[derive(TypePath)]
/// #[type_path = "crate_name::foo::C"]
/// struct C<T>(T);
/// ```
#[proc_macro_derive(TypePath, attributes(type_path))]
pub fn derive_type_path(input: TokenStream) -> TokenStream {
    use crate::derive_data::{ReflectMeta, TypeAttributes, TypeSignature};

    let ast: DeriveInput = parse_macro_input!(input as DeriveInput);

    let type_attributes = match TypeAttributes::parse_type_path(&ast.attrs) {
        Ok(v) => v,
        Err(err) => return err.into_compile_error().into(),
    };

    let type_parser =
        TypeSignature::new_local(&ast.ident, type_attributes.type_path.clone(), &ast.generics);

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
///     #[type_path = "core::option:Option"]
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
/// impl_reflect_opaque!(u64 (Clone, Debug, Hash, PartialEq, PartialOrd, Default, Serialize, Deserialize));
/// impl_reflect_opaque!(::utils::One<T: Clone> (Clone));
/// impl_reflect_opaque!(::alloc::string::String (Clone, Debug, docs = "hello"));
/// impl_reflect_opaque!((in core::time) Instant (Clone));
/// impl_reflect_opaque!((in core::time as Ins) Instant (Clone));
/// ```
///
/// This macro always implies `Opaque`, so `Clone` is required.
///
/// See available attributes in [`derive Reflect`](derive_full_reflect) .
#[proc_macro]
pub fn impl_reflect_opaque(input: TokenStream) -> TokenStream {
    use crate::derive_data::{ReflectMeta, ReflectOpaqueParser, TypeSignature};

    let ReflectOpaqueParser {
        attrs,
        custom_path,
        type_ident,
        type_path,
        generics,
    } = parse_macro_input!(input with ReflectOpaqueParser::parse);

    let parser = TypeSignature::new_foreign(&type_ident, &type_path, custom_path, &generics);

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
    use crate::derive_data::{ReflectMeta, ReflectTypePathParser, TypeAttributes, TypeSignature};

    let ReflectTypePathParser {
        custom_path,
        type_ident,
        type_path,
        generics,
    } = parse_macro_input!(input with ReflectTypePathParser::parse);

    let parser = TypeSignature::new_foreign(&type_ident, &type_path, custom_path, &generics);

    let meta = ReflectMeta::new(TypeAttributes::default(), parser);

    let type_path_impls = impls::impl_trait_type_path(&meta);

    quote! {
        const _: () = {
            #type_path_impls
        };
    }
    .into()
}

/// Registers a concrete type for automatic discovery via the reflection system.
///
/// This macro is intended for types **defined in the current crate** (local types).
/// It leverages the `AutoRegister` trait to prevent duplicate registrations at compile time.
///
/// # Duplicate Registration Prevention
/// - The macro generates an `impl AutoRegister for #type_path` block.
/// - If the same type is registered multiple times, the compiler will complain about
///   conflicting implementations of `AutoRegister`, effectively preventing duplicates.
/// - This is the recommended macro for most use cases.
///
/// # Feature Flag
/// If the required feature is disabled, this macro expands to nothing.
///
/// # Requirements
/// - The type must be **concrete** (no unbound generic parameters).
///   - ✅ `foo::Foo`
///   - ✅ `Vec<u32>`
///   - ❌ `Vec<T>` (where `T` is unconstrained)
/// - The type must be defined in the current crate (to satisfy the orphan rule).
///
/// # Example
/// ```ignore
/// impl_auto_register!(foo::Foo);
/// impl_auto_register!(Bar<u32>);           // OK - concrete type
/// impl_auto_register!(Bar<T>);             // Error - generic parameters
/// ```
///
/// # For Remote Types
/// For types defined in other crates, use [`auto_register!`] instead, as the orphan rule
/// prevents implementing `AutoRegister` for external types.
///
/// See also: [`derive Reflect`](derive_full_reflect)
#[proc_macro]
pub fn impl_auto_register(input: TokenStream) -> TokenStream {
    let type_path = syn::parse_macro_input!(input as syn::Type);

    let voker_reflect_path = path::voker_reflect();
    let macro_utils_ = path::macro_utils_(&voker_reflect_path);

    TokenStream::from(quote! {
        const _: () = {
            impl #macro_utils_::AutoRegister for #type_path {}

            #macro_utils_::inv::submit!{
                #macro_utils_::RegisterFn::of::<#type_path>()
                => #macro_utils_::RegisterFn
            }
        };
    })
}

/// Registers a concrete type for automatic discovery, **without** compile-time duplicate prevention.
///
/// This macro is designed for **remote types** (defined in other crates) where implementing
/// the `AutoRegister` trait is impossible due to the orphan rule.
///
/// # Duplicate Registration Behavior
/// - Unlike [`impl_auto_register!`], this macro does **not** generate an `AutoRegister`
///   implementation, and thus cannot prevent duplicate registrations at compile time.
/// - Duplicate registrations are **safe** and will not cause runtime errors.
/// - The only downside is a **minor performance impact during iteration** over registered types
///   (duplicates will be visited multiple times).
///
/// # When to Use
/// - ✅ Types from external crates (e.g., `std::string::String`, `serde_json::Value`)
/// - ✅ When you cannot or don't want to implement `AutoRegister` for a type
/// - ❌ Avoid for local types—use [`impl_auto_register!`] instead for better compile-time checking
///
/// # Feature Flag
/// If the required feature is disabled, this macro expands to nothing.
///
/// # Requirements
/// - The type must be **concrete** (no unbound generic parameters).
///
/// # Example
///
/// ```ignore
/// auto_register!(std::string::String);      // OK - remote type
/// auto_register!(Vec<u32>);                // OK - concrete, but consider impl_auto_register! for local
/// auto_register!(Vec<T>);                  // Error - generic parameters
/// ```
///
/// # Note on Performance
/// Duplicate registrations slightly increase iteration time but have no other side effects.
/// In practice, this is negligible unless thousands of duplicates are registered.
///
/// See also: [`derive Reflect`](derive_full_reflect)
#[proc_macro]
pub fn auto_register(input: TokenStream) -> TokenStream {
    let type_path = syn::parse_macro_input!(input as syn::Type);

    let voker_reflect_path = path::voker_reflect();
    let macro_utils_ = path::macro_utils_(&voker_reflect_path);

    TokenStream::from(quote! {
        const _: () = {
            #macro_utils_::inv::submit!{
                #macro_utils_::RegisterFn::of::<#type_path>()
                => #macro_utils_::RegisterFn
            }
        };
    })
}

/// Impl `TypeData` for specific trait with a new struct.
///
/// This macro will generate a `{trait_name}FromReflect`(default) struct,
/// which implements `TypeData` and `TypePath`. For example, for `Display`,
/// this will generate `DisplayFromReflect`.
///
/// It only contains three methods internally:
/// - `from_ref`: cast `&dyn Reflect` to `&dyn {trait_name}`
/// - `from_mut`: cast `&mut dyn Reflect` to `&mut dyn {trait_name}`
/// - `from_boxed`: cast `Box<dyn Reflect>` to `Box<dyn {trait_name}>`
///
/// You can specify the generated type name through `name = Name`
/// (e.g. `#[reflect_trait(name = MyDisplay)]`).
///
/// ## Example
///
/// ```ignore
/// #[reflect_trait(name = MyDebugAdapter)]
/// pub trait MyDebug {
///     fn debug(&self);
/// }
///
/// impl MyDebug for String { /* ... */ }
///
/// let reg = TypeRegistry::new()
///     .register::<String>()
///     .register_type_data::<String, MyDebugAdapter>();
///
/// let x: Box<dyn Reflect> = Box::new(String::from("123"));
///
/// let my_debug_from = reg.get_type_data::<MyDebugAdapter>((*x).type_id()).unwrap();
/// let x: Box<dyn MyDebug> = my_debug_from.from_boxed(x);
/// x.debug();
/// ```
#[proc_macro_attribute]
pub fn reflect_trait(args: TokenStream, input: TokenStream) -> TokenStream {
    impls::impl_reflect_trait(args, input)
}
