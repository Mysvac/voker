#![expect(clippy::module_inception, reason = "For better structure.")]

use super::{Required, StorageMode};
use crate::clone::ComponentCloner;
use crate::component::hook::ComponentHook;
use crate::entity::EntityMapper;
use crate::relationship::RelationshipRegistrar;
use crate::utils::Dropper;

// -----------------------------------------------------------------------------
// Component

/// The core trait for all component types.
///
/// Any type stored in ECS component storage must implement this trait.
///
/// `Component` describes runtime metadata that drives how ECS stores and manages
/// values of this type: storage layout, mutability, clone/drop behavior,
/// dependency expansion, lifecycle hooks, and relationship registration.
///
/// Most users should not implement this trait manually. Prefer deriving it with
/// [Component derive macro], which sets sensible defaults and validates options.
///
/// # Derive Macro
///
/// For most component types, prefer using the [Component derive macro].
///
/// ```no_run
/// # use voker_ecs::derive::Component;
/// // Basic usage - mutable component
/// #[derive(Component, Clone, Default)]
/// struct Foo;
///
/// // Component with required dependencies
/// #[derive(Component, Clone)]
/// #[require(Foo)]
/// struct Baz;
///
/// // Immutable component with sparse storage
/// #[derive(Component, Clone, Default)]
/// #[component(mutable = false, storage = "sparse")]
/// struct Logger { /* .. */ }
///
/// // Combined: copyable, immutable, with multiple required dependencies
/// #[derive(Component, Clone, Copy)]
/// #[component(Copy, mutable = false)]
/// #[require(Foo, Logger)]
/// struct GameVersion<T: Copy>(T);
/// ```
///
/// [Component derive macro]: crate::derive::Component
///
/// # Associated Constants
///
/// These constants define component behavior at compile time.
///
/// - [`Component::CLONER`]
///   Function pointer used by ECS clone paths (entity/world clone operations) to
///   duplicate component values. By default, components are required to support cloning.
///   See more information in [`ComponentCloner`](crate::clone::ComponentCloner).
///
/// - [`Component::STORAGE`]
///   Storage strategy for this component. Use [`StorageMode::Dense`] for table
///   columns or [`StorageMode::Sparse`] for sparse maps.
///
/// - [`Component::MUTABLE`]
///   Whether mutable access is allowed through ECS APIs.
///   When `false`, mutable fetch/query operations fail for this component.
///
/// - [`Component::NO_ENTITY`]
///   Optimization flag indicating that [`Component::map_entities`] is a no-op.
///   Set to `true` only when the component never stores any entity references.
///
/// - [`Component::DROPPER`]
///   Optional drop function pointer used by erased storage to destroy values.
///   The default (`Dropper::of::<Self>()`) is correct for almost all cases.
///
/// - [`Component::REQUIRED`]
///   Optional dependency set for automatic required-component insertion.
///   If component `A` requires `B`, inserting/spawning `A` also inserts `B`
///   (via `Default`) when missing.
///
/// - Lifecycle hooks: [`Component::ON_ADD`], [`Component::ON_CLONE`],
///   [`Component::ON_INSERT`], [`Component::ON_REMOVE`],
///   [`Component::ON_DISCARD`], [`Component::ON_DESPAWN`]
///   Optional callbacks invoked at specific lifecycle transitions.
///
/// - [`Component::RELATIONSHIP_REGISTRAR`]
///   Optional relationship metadata registrar for link/relationship components.
///
/// When using derive, these are configured with attributes such as
/// `#[component(storage = "sparse")]`, `#[component(mutable = false)]`, and
/// `#[require(Foo)]`.
///
/// ## Required Components
///
/// Any component used in [`Component::REQUIRED`] must implement [`Default`].
///
/// Multiple required components are supported via tuples, for example:
/// - `const REQUIRED: Option<Required> = Some((A, B, C, D));`
///
/// ## Hooks
///
/// Lifecycle hooks allow components to react to ECS transitions.
///
/// - Entity spawn: `on_add -> on_insert`
/// - Entity despawn: `on_despawn -> on_discard -> on_remove`
/// - Component insert: `on_discard` (replaced) -> `on_add` (new) -> `on_insert`
/// - Component remove: `on_discard -> on_remove`
/// - Entity clear: `on_discard -> on_remove`
/// - Entity clone: `on_clone -> on_add -> on_insert`
///
/// See [`ComponentHooks`] for more information.
///
/// [`ComponentHooks`]: crate::component::ComponentHooks
///
/// # Safety
///
/// This trait is safe to implement, but incorrect metadata can break ECS
/// invariants and lead to severe runtime bugs. Derive is strongly recommended.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a component",
    label = "invalid component",
    note = "Consider annotating `{Self}` with `#[derive(Component)]`."
)]
pub trait Component: Sized + Send + Sync + 'static {
    /// Clone/copy callback used by ECS clone paths.
    const CLONER: ComponentCloner;

    /// Storage mode for this component type.
    ///
    /// Defaults to [`StorageMode::Dense`].
    const STORAGE: StorageMode = StorageMode::Dense;

    /// Whether mutable access is allowed.
    ///
    /// Defaults to `true`.
    const MUTABLE: bool = true;

    /// Whether [`Component::map_entities`] can be skipped.
    ///
    /// Set this to `true` only when the component contains no entity references.
    /// Defaults to `false`.
    const NO_ENTITY: bool = false;

    /// Drop callback for values stored behind type-erased pointers.
    ///
    /// The default uses [`Dropper::of`] for this component type.
    const DROPPER: Option<Dropper> = Dropper::of::<Self>();

    /// Optional set of required components auto-inserted with this component.
    ///
    /// Defaults to `None`.
    const REQUIRED: Option<Required> = None;

    /// Hook invoked when this component is added to an entity.
    const ON_ADD: Option<ComponentHook> = None;

    /// Hook invoked when this component is cloned as part of entity cloning.
    const ON_CLONE: Option<ComponentHook> = None;

    /// Hook invoked after this component is inserted.
    const ON_INSERT: Option<ComponentHook> = None;

    /// Hook invoked when this component is removed.
    const ON_REMOVE: Option<ComponentHook> = None;

    /// Hook invoked when this component value is replaced or discarded.
    const ON_DISCARD: Option<ComponentHook> = None;

    /// Hook invoked when the owner entity is despawned.
    const ON_DESPAWN: Option<ComponentHook> = None;

    /// Optional relationship metadata registrar for link-like components.
    const RELATIONSHIP_REGISTRAR: Option<RelationshipRegistrar> = None;

    /// Remaps embedded entity references after entity-ID migration.
    ///
    /// Override this when the component stores [`crate::entity::Entity`] values.
    #[inline(always)]
    #[expect(unused_variables, reason = "default implementation")]
    fn map_entities<E: EntityMapper>(this: &mut Self, mapper: &mut E) {}
}
