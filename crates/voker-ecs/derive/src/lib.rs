#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(clippy::std_instead_of_core, reason = "proc-macro lib")]
#![allow(clippy::std_instead_of_alloc, reason = "proc-macro lib")]

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

// -----------------------------------------------------------------------------
// Modules

mod bundle;
mod component;
mod entity_event;
mod event;
mod game_error;
mod message;
mod path;
mod query_data;
mod resource;
mod schedule;
mod system_param;
mod utils;

// -----------------------------------------------------------------------------
// Macros

/// Derives the `Resource` trait implementation.
///
/// This macro automatically implements the `Resource` trait for your type,
/// allowing it to be used as a global resource in the ECS system.
///
/// # Examples
///
/// ```ignore
/// // Basic usage - mutable resource
/// #[derive(Resource)]
/// struct Foo;
///
/// // Immutable resource
/// #[derive(Resource)]
/// #[resource(mutable = false)]
/// struct Logger { /* .. */ }
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
/// # Examples
///
/// ```ignore
/// #[derive(Component, Clone)]
/// #[component(storage = "sparse", mutable = false)]
/// struct Logger;
///
/// #[derive(Component, Clone, Default)]
/// #[component(required = (A, B))]
/// struct NeedsDeps;
///
/// #[derive(Component, Clone, Copy)]
/// #[component(Copy)]
/// struct Version(u32);
///
/// #[derive(Component, Clone)]
/// #[relationship(relationship_target = Children)]
/// struct ChildOf {
///     #[relationship]
///     target: Entity,
/// }
///
/// #[derive(Component, Default)]
/// #[relationship(relationship = ChildOf, linked_lifecycle)]
/// struct Children {
///     #[relationship]
///     sources: Vec<Entity>,
/// }
/// ```
///
/// # Optional Parameters
///
/// This derive supports three attribute groups:
/// - `#[component(...)]`
/// - `#[relationship(...)]`
/// - `#[relationship_target(...)]`
///
/// Parameters are listed below in the same order as the parser.
///
/// ## `#[component(...)]`
///
/// - `storage = "dense" | "sparse"`
///   Controls `Component::STORAGE`. Default is `"dense"`.
///
/// - `mutable` or `mutable = true | false`
///   Controls `Component::MUTABLE`. Default is `true`.
///
/// - `no_entity` or `no_entity = true | false`
///   Controls `Component::NO_ENTITY`.
///   If omitted, derive may still set `NO_ENTITY = true` automatically when no
///   mappable entity fields are detected.
///
/// - `required = T`
///   Sets `Component::REQUIRED` to `Required::from::<T>()`.
///   `T` can be a single type or tuple. Required component types should support
///   default construction for insertion paths.
///
/// - `on_add` or `on_add = path`
/// - `on_clone` or `on_clone = path`
/// - `on_insert` or `on_insert = path`
/// - `on_remove` or `on_remove = path`
/// - `on_discard` or `on_discard = path`
/// - `on_despawn` or `on_despawn = path`
///   Hook entries. A bare key uses `Self::on_xxx` as default path.
///
/// - `cloner = path`
///   Uses a custom cloner callback (`ComponentCloner::custom(path)`).
///
/// - `Clone`
///   Uses clone-based cloner (`ComponentCloner::clonable::<Self>()`).
///
/// - `Copy`
///   Uses copy-based cloner (`ComponentCloner::copyable::<Self>()`).
///
/// Cloner behavior note:
/// - For regular components (not `relationship` / `relationship_target`), the
///   default cloner is clone-based.
/// - This means component types are generally expected to support cloning.
/// - For special types that cannot be safely cloned, use `cloner = path` and
///   handle clone-time cleanup in `on_clone` (for example, removing the cloned
///   component immediately after clone).
///
/// ## `#[relationship(...)]`
///
/// Enables `Relationship` implementation and relationship-aware component
/// behavior.
///
/// - `relationship_target = T` (required)
///   Target component type.
///
/// - `allow_self_referential` or `allow_self_referential = true | false`
///   Controls `Relationship::ALLOW_SELF_REFERENTIAL`. Default is `false`.
///
/// Additional rules:
/// - Relationship sets a relationship-specific default cloner.
/// - Custom `on_insert` and `on_discard` hooks are rejected because
///   relationship derive provides these hooks.
///
/// ## `#[relationship_target(...)]`
///
/// Enables `RelationshipTarget` implementation and target-aware component
/// behavior.
///
/// - `relationship = T` (required)
///   Source relationship component type.
///
/// - `linked_lifecycle` or `linked_lifecycle = true | false`
///   Controls `RelationshipTarget::LINKED_LIFECYCLE`. Default is `false`.
///
/// Additional rules:
/// - RelationshipTarget sets a target-specific default cloner.
/// - Custom `on_discard` and `on_despawn` hooks are rejected because
///   relationship target derive provides these hooks.
///
/// ## Field Attributes For Entity Mapping
///
/// - `#[entities]` on fields: include that field in generated `map_entities`.
/// - `#[related]` on one struct field: marks the relationship payload field.
#[proc_macro_derive(
    Component,
    attributes(component, relationship, relationship_target, related)
)]
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

/// Derives conversion from your error type into `GameError`.
///
/// This macro generates:
/// - `impl From<T> for GameError`
///
/// Internally, the generated `From` impl calls `GameError::new(severity, value)`.
///
/// # Optional Attributes
///
/// - `#[game_error(severity = "ignore" | "trace" | "debug" | "info" | "warning" | "error" | "panic")]`
///   Uses a fixed severity for this type.
///
/// - `#[game_error(severity = self.field)]` or `#[game_error(severity = self.0)]`
///   Uses a severity value stored in the error instance.
///
/// If no attribute is provided, severity defaults to `Severity::Panic`.
///
/// # Type Requirement
///
/// The generated conversion passes your value into `GameError::new`, which
/// requires `Box<dyn Error + Send + Sync>: From<T>`.
///
/// In practice, this usually means your type implements `std::error::Error`
/// (and is `Send + Sync + 'static`), or otherwise has a compatible conversion
/// into `Box<dyn Error + Send + Sync>`.
///
/// # Examples
///
/// ```ignore
/// use core::error::Error;
/// use core::fmt::{self, Display};
///
/// #[derive(Debug, GameError)]
/// #[game_error(severity = "warning")]
/// struct ConfigError;
///
/// impl Display for ConfigError {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         write!(f, "invalid config")
///     }
/// }
///
/// impl Error for ConfigError {}
/// ```
///
/// ```ignore
/// use core::error::Error;
/// use core::fmt::{self, Display};
///
/// #[derive(Debug, GameError)]
/// #[game_error(severity = self.severity)]
/// struct SystemError {
///     severity: Severity,
///     msg: &'static str,
/// }
///
/// impl Display for SystemError {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         write!(f, "{}", self.msg)
///     }
/// }
///
/// impl Error for SystemError {}
/// ```
#[proc_macro_derive(GameError, attributes(game_error))]
pub fn derive_game_error(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    game_error::impl_derive_game_error(ast)
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
/// This macro implements `Message` for the annotated type.
///
/// # Requirements
///
/// Generated impls require the target type to satisfy message trait bounds,
/// effectively meaning `Send + Sync + 'static`.
///
/// # Example
///
/// ```ignore
/// use voker_ecs::prelude::*;
///
/// #[derive(Message)]
/// struct DamageApplied {
///     entity: u32,
///     amount: u32,
/// }
///
/// fn log_damage(mut reader: MessageReader<DamageApplied>) {
///     for msg in reader.read() {
///         let _ = (msg.entity, msg.amount);
///     }
/// }
/// ```
#[proc_macro_derive(Message)]
pub fn derive_message(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    message::impl_derive_message(ast)
}

/// Derives the `QueryData` trait implementation.
///
/// Supports structs whose fields are themselves query-data entries.
///
/// Generated implementation composes all field states/caches/fetch paths.
///
/// # Lifetime requirements
///
/// The struct may be either:
/// - non-generic over lifetimes, or
/// - generic over exactly one lifetime named `'w`.
///
/// Any other lifetime shape is rejected.
///
/// # Read-only marker
///
/// Add `#[query_data(readonly)]` to also derive `ReadOnlyQueryData`.
/// This is required when using `.iter()` (read-only query iteration).
///
/// # Mutable field rule
///
/// Do not use `&'w mut T` in fields of a derived `QueryData` struct.
/// Use `Mut<'w, T>` instead.
///
/// # Examples
///
/// ```ignore
/// use voker_ecs::borrow::Mut;
/// use voker_ecs::derive::QueryData;
/// use voker_ecs::prelude::*;
///
/// #[derive(QueryData)]
/// #[query_data(readonly)]
/// struct ReadVelocity<'w> {
///     velocity: &'w Velocity,
/// }
///
/// #[derive(QueryData)]
/// struct MoveData<'w> {
///     position: Mut<'w, Position>,
///     velocity: &'w Velocity,
/// }
/// ```
#[proc_macro_derive(QueryData, attributes(query_data))]
pub fn derive_query_data(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    query_data::impl_derive_query_data(ast)
}

/// Derives the `SystemParam` trait implementation.
///
/// Supports structs whose fields are themselves `SystemParam` entries.
///
/// # What The Derive Generates
///
/// For a struct `P<'w, 's>`, this derive generates an unsafe
/// `impl SystemParam for P<'_, '_>` that:
/// - composes field states into one tuple `State`,
/// - forwards access registration to all fields,
/// - fetches each field value during system execution and reconstructs `P`.
///
/// This means the derived parameter behaves like a thin composition layer over
/// existing parameter implementations.
///
/// # Lifetime requirements
///
/// The struct must declare exactly two lifetimes named `'w` and `'s`.
/// Any other lifetime shape is rejected.
///
/// Lifetime meanings:
/// - `'w`: data borrowed from `World` during one system run.
/// - `'s`: data borrowed from the system-local parameter state.
///
/// # Field requirements
///
/// Every field type must implement `SystemParam`.
///
/// Typical field examples:
/// - `Res<'w, T>`
/// - `Local<'s, T>`
/// - `Commands<'w, 's>`
/// - `Query<'w, 's, ...>`
///
/// # Example
///
/// ```ignore
/// use voker_ecs::borrow::Res;
/// use voker_ecs::command::Commands;
/// use voker_ecs::derive::{Resource, SystemParam};
/// use voker_ecs::system::Local;
///
/// #[derive(Resource)]
/// struct Counter(u32);
///
/// #[derive(SystemParam)]
/// struct CounterParam<'w, 's> {
///     counter: Res<'w, Counter>,
///     local: Local<'s, u32>,
///     commands: Commands<'w, 's>,
/// }
/// ```
///
/// # Notes
///
/// This derive currently targets concrete struct composition.
/// If a field's `SystemParam::Item<'w, 's>` cannot be inferred as the same
/// field shape expected by the struct definition, implement `SystemParam`
/// manually for the outer type.
///
/// If one of the required lifetimes (`'w` / `'s`) is not used by real fields,
/// Rust may emit an "unused lifetime parameter" warning.
/// You can keep the lifetime explicit by adding a marker field such as:
/// `PhantomData<(&'w (), &'s ())>`.
///
/// ```ignore
/// use core::marker::PhantomData;
/// use voker_ecs::derive::SystemParam;
///
/// #[derive(SystemParam)]
/// struct MarkerOnly<'w, 's> {
///     marker: PhantomData<(&'w (), &'s ())>,
/// }
/// ```
#[proc_macro_derive(SystemParam)]
pub fn derive_system_param(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    system_param::impl_derive_system_param(ast)
}

/// Derives the `Event` trait implementation.
///
/// ```ignore
/// use voker_ecs::derive::Event;
///
/// /// Defaults to GlobalTrigger.
/// #[derive(Event)]
/// struct A;
///
/// /// Custom trigger.
/// #[derive(Event)]
/// #[event(trigger = CustomTrigger)]
/// struct B;
///
/// /// Custom trigger with lifetime.
/// #[derive(Event)]
/// #[event(trigger = TriggerWithLifetime<'a>)]
/// struct C;
/// ```
#[proc_macro_derive(Event, attributes(event))]
pub fn derive_event(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    event::impl_derive_event(ast)
}

/// Derives the `EntityEvent` trait implementation.
///
/// ```ignore
/// use voker_ecs::derive::Event;
///
/// #[derive(EntityEvent)]
/// struct A(Entity);
///
/// #[derive(EntityEvent)]
/// struct B {
///     /// Explicitly specify entity field.
///     #[event_target]
///     target: Entity,
/// }
///
/// /// Custom entity trigger.
/// #[derive(EntityEvent)]
/// #[entity_event(trigger = CustomTrigger)]
/// struct A(Entity);
///
/// #[derive(EntityEvent)]
/// /// Enable propagation, which defaults to using the ChildOf component
/// #[entity_event(propagate)]
/// /// Enable propagation using the given Traversal implementation
/// #[entity_event(propagate = &'static ChildOf)]
/// /// Always propagate
/// #[entity_event(auto_propagate)]
/// struct A(Entity);
/// ```
#[proc_macro_derive(EntityEvent, attributes(entity_event, event_target))]
pub fn derive_entity_event(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    entity_event::impl_derive_entity_event(ast)
}
