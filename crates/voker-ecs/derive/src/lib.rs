#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(clippy::std_instead_of_core, reason = "proc-macro lib")]
#![allow(clippy::std_instead_of_alloc, reason = "proc-macro lib")]

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

// -----------------------------------------------------------------------------
// Modules

mod bundle;
mod component;
mod message;
mod path;
mod resource;
mod schedule;
mod utils;

// -----------------------------------------------------------------------------
// Macros

/// Derives the `Resource` trait implementation.
///
/// This macro automatically implements the `Resource` trait for your type,
/// allowing it to be used as a global resource in the ECS system.
///
/// # Supported Attributes
///
/// The `#[resource(...)]` attribute can be used to configure the resource behavior:
///
/// | Attribute | Description | Default |
/// |-----------|-------------|---------|
/// | `copy` / `clone` | Sets the cloning behavior. | Not cloneable |
/// | `mutable = true/false` | Controls whether the resource can be mutated | `true` |
///
/// # Examples
///
/// ```ignore
/// // Basic usage - mutable resource without clone capability
/// #[derive(Resource)]
/// struct Foo;
///
/// // Cloneable resource
/// #[derive(Resource, Clone)]
/// #[resource(Clone)]
/// struct Bar(String);
///
/// // Immutable resource
/// #[derive(Resource)]
/// #[resource(mutable = false)]
/// struct Logger { /* .. */ }
///
/// // Combined: copyable and immutable
/// #[derive(Resource, Clone, Copy)]
/// #[resource(Copy, mutable = false)]
/// struct GameVersion<T: Copy>(T);
/// ```
#[proc_macro_derive(Resource, attributes(resource))]
pub fn derive_resource(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    resource::impl_derive_resource(ast)
}

/// Derives the `Component` trait implementation.
///
/// This macro automatically implements the `Component` trait for your type,
/// allowing it to be used as a component in the ECS system.
///
///
///
/// # Supported Attributes
///
/// The `#[component(...)]` attribute can be used to configure the component behavior:
///
/// | Attribute | Description | Default |
/// |-----------|-------------|---------|
/// | `Copy` | Mark the type is copyable | Clone through `Clone` |
/// | `mutable = true/false` | Controls whether the component can be mutated | `true` |
/// | `storage = "dense"/"sparse"` | Controls how the component is stored in memory | `"dense"` |
/// | `required = T` | Specifies dependency components. `T` can be a single type or a tuple of types | `()` |
///
/// **Note**: Components used in `required` must implement the `Default` trait.
///
/// # Examples
///
/// ```ignore
/// // Basic usage - mutable and cloneable component
/// #[derive(Component, Clone)]
/// struct Foo;
///
///
/// // Component with required dependencies
/// #[derive(Component, Clone)]
/// #[component(required = Bar)]
/// struct Baz;
///
/// // Immutable component with sparse storage
/// #[derive(Component, Clone, Default)]
/// #[component(mutable = false, storage = "sparse")]
/// struct Logger { /* .. */ }
///
/// // Combined: copyable, immutable, with multiple required dependencies
/// #[derive(Component, Clone, Copy)]
/// #[component(Copy, mutable = false, required = (Foo, Bar))]
/// struct GameVersion<T: Copy>(T);
/// ```
#[proc_macro_derive(Component, attributes(component))]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    component::impl_derive_component(ast)
}

/// Derives the `Bundle` trait implementation.
///
/// This macro automatically implements the `Bundle` trait for your struct,
/// allowing it to be used as a bundle when spawning entities. All fields must
/// implement `Bundle` (or `Component`, which automatically implements `Bundle`).
///
/// # Behavior
///
/// - Each field in the struct represents a sub-bundle that will be combined
/// - Components from all fields are merged when spawning entities
/// - If duplicate components exist across fields, later fields override earlier ones
/// - The `()` (unit) type can be used for empty bundles
///
/// # Examples
///
/// ```ignore
/// #[derive(Component, Clone)]
/// struct Foo;
///
/// #[derive(Component, Clone)]
/// struct Bar(u8);
///
/// #[derive(Component, Clone)]
/// struct Baz(String);
///
/// // Empty bundle - spawns an entity with no components
/// #[derive(Bundle)]
/// struct EmptyBundle {}
///
/// // Regular bundle - equivalent to `(Foo, Bar)` when spawning
/// #[derive(Bundle)]
/// struct MyBundle {
///     a: Foo,
///     b: Bar,
/// }
///
/// // Bundle with duplicate components
/// // Later fields override earlier ones when spawning
/// // No memory leaks occur
/// #[derive(Bundle)]
/// struct OverrideBundle {
///     first: Baz,
///     second: Baz,  // This value will override `first` for the same component type
/// }
/// ```
#[proc_macro_derive(Bundle)]
pub fn derive_bundle(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    bundle::impl_derive_bundle(ast)
}

/// Derives the `ScheduleLabel` trait implementation.
///
/// # Required Traits
///
/// The target type must implement the following traits:
/// - `Clone`
/// - `Debug`
/// - `Hash`
/// - `Eq`
///
/// # Examples
///
/// ```ignore
/// #[derive(ScheduleLabel, Clone, Debug, Hash, PartialEq, Eq)]
/// enum GameStage {
///     Begin,
///     Input,
///     Physics,
///     Logic,
///     Animation,
///     Collision,
///     Render,
///     End,
/// }
/// ```
#[proc_macro_derive(ScheduleLabel)]
pub fn derive_schedule_label(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    schedule::impl_derive_schedule_label(ast)
}

/// Derives the `Message` trait implementation.
///
/// # Examples
///
/// ```ignore
/// #[derive(Message)]
/// struct GameStage {
///     text: String
/// }
/// ```
#[proc_macro_derive(Message)]
pub fn derive_message(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    message::impl_derive_message(ast)
}

/// Derives the `QueryData` trait implementation.
///
/// TODO
#[proc_macro_derive(QueryData)]
pub fn derive_query_data(input: TokenStream) -> TokenStream {
    let _ast = parse_macro_input!(input as DeriveInput);
    todo!()
}

/// Derives the `SystemParam` trait implementation.
///
/// TODO
#[proc_macro_derive(SystemParam)]
pub fn derive_system_param(input: TokenStream) -> TokenStream {
    let _ast = parse_macro_input!(input as DeriveInput);
    todo!()
}
