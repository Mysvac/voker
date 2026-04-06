//! Core component trait definition for the entity-component system.
//!
//! This module defines the fundamental `Component` trait that all components
//! must implement, along with associated configuration constants that control
//! component behavior within the system.

use super::{Required, StorageMode};
use crate::component::hook::ComponentHook;
use crate::entity::EntityMapper;
use crate::utils::{Cloner, Dropper};

// -----------------------------------------------------------------------------
// ComponentHook

// -----------------------------------------------------------------------------
// Component

/// The core trait for all components in the entity-component system.
///
/// This trait must be implemented for any type that can be used as a component.
/// It provides essential metadata about the component's behavior, including
/// mutability, storage strategy, cloning behavior, and required dependencies.
///
/// # Derive Macro
///
/// For most component types, prefer using the [Component derive macro].
///
/// ```no_run
/// # use voker_ecs::derive::Component;
/// // Basic usage - mutable component without clone capability
/// #[derive(Component, Default)]
/// struct Foo;
///
/// // A clonable component
/// #[derive(Component, Clone, Default)]
/// struct Bar(String);
///
/// // Component with required dependencies
/// #[derive(Component)]
/// #[component(required = Bar)]
/// struct Baz;
///
/// // Immutable component with sparse storage
/// #[derive(Component, Default)]
/// #[component(mutable = false, storage = "sparse")]
/// struct Logger { /* .. */ }
///
/// // Combined: copyable, immutable, with multiple required dependencies
/// #[derive(Component, Clone, Copy)]
/// #[component(cloner = "copy", mutable = false, required = (Foo, Bar))]
/// struct GameVersion<T: Copy>(T);
/// ```
///
/// [Component derive macro]: crate::derive::Component
///
/// # Features
///
/// ## Storage
///
/// Two storage strategies are supported: `dense` and `sparse`, configured via
/// [`Component::STORAGE`].
///
/// When using the derive macro, you can set storage with
/// `#[component(storage = "dense/sparse")]`.
///
/// See [`StorageMode`] for implementation details.
///
/// ## Mutable
///
/// Components are mutable by default, but can be made immutable with
/// [`Component::MUTABLE`].
///
/// When using the derive macro, mutability can be configured via
/// `#[component(mutable = true/false)]`.
///
/// If a component is immutable, APIs such as `get_mut` and `fetch` cannot return
/// mutable references (they return `None`). A mutable `Query` access instead
/// returns an error, which by default may lead to a panic.
///
/// ## Required
///
/// Dependency components are configured via [`Component::REQUIRED`], which
/// defaults to `None`.
///
/// Required components act like dependencies. If component `A` requires `B`,
/// then spawning or inserting `A` will automatically add `B` via [`Default`]
/// when `B` is missing.
///
/// Any component used as a required dependency must implement [`Default`].
///
/// Multiple required components are supported via tuples, for example:
/// - `const REQUIRED: Option<Required> = Some((A, B, C, D));`
///
/// With the derive macro, use `#[component(required = T)]`.
///
/// ## Dropper
///
/// [`Component::DROPPER`] stores the function pointer for [`Drop::drop`].
///
/// [`Dropper`] extracts this pointer at compile time, so users usually do not
/// need to specify it manually.
///
/// # Safety
///
/// Although this trait is not declared `unsafe`, incorrect implementations can
/// still cause serious bugs, including:
/// - Memory unsafety in component storage and access
/// - Violation of thread safety guarantees
/// - Incorrect component versioning and tick tracking
/// - Undefined behavior in component cloning and mutation
///
/// The default provided configuration is safe.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a component",
    label = "invalid component",
    note = "Consider annotating `{Self}` with `#[derive(Component)]`."
)]
pub trait Component: Clone + Send + Sync + 'static {
    /// The storage type of component, default is `Dense`.
    const STORAGE: StorageMode = StorageMode::Dense;

    /// The mutability of the component, default is `true`.
    const MUTABLE: bool = true;

    /// The function pointer of [`Drop`].
    const DROPPER: Option<Dropper> = Dropper::of::<Self>();

    const CLONER: Cloner = Cloner::clonable::<Self>();

    /// The required components, default is `None`.
    const REQUIRED: Option<Required> = None;

    const ON_ADD: Option<ComponentHook> = None;
    const ON_CLONE: Option<ComponentHook> = None;
    const ON_INSERT: Option<ComponentHook> = None;
    const ON_REMOVE: Option<ComponentHook> = None;
    const ON_DISCARD: Option<ComponentHook> = None;
    const ON_DESPAWN: Option<ComponentHook> = None;

    #[inline(always)]
    #[expect(unused_variables, reason = "default implementation")]
    fn map_entities(this: &mut Self, mapper: &mut dyn EntityMapper) {}
}
