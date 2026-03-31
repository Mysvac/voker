use crate::utils::Dropper;

/// Marker trait for values stored in a world's resource storage.
///
/// A resource is a singleton value identified by its concrete Rust type. At most
/// one value of a given resource type can exist in a [`World`].
/// Thread-safety determines which access APIs are available:
///
/// - `Send + Sync` resources can be accessed through [`crate::borrow::Res`],
///   [`ResRef`], and [`crate::borrow::ResMut`].
/// - `!Sync` resources must stay on the main thread and are accessed through
///   [`NonSend`], [`NonSendRef`], and [`NonSendMut`].
///
/// [`World`]: crate::world::World
/// [`Res`]: crate::borrow::Res
/// [`ResRef`]: crate::borrow::ResRef
/// [`ResMut`]: crate::borrow::ResMut
/// [`NonSend`]: crate::borrow::NonSend
/// [`NonSendRef`]: crate::borrow::NonSendRef
/// [`NonSendMut`]: crate::borrow::NonSendMut
///
/// # Derive Macro
///
/// For most component types, prefer using the [Resource derive macro].
///
/// ```no_run
/// # use voker_ecs::derive::Resource;
/// // Basic usage - mutable resource without clone capability
/// #[derive(Resource)]
/// struct Foo;
///
/// // Immutable resource
/// #[derive(Resource)]
/// #[resource(mutable = false)]
/// struct Logger { /* .. */ }
/// ```
///
/// [Resource derive macro]: crate::derive::Resource
///
/// # Features
///
/// ## Mutable
///
/// Resources are mutable by default, but can be made immutable with
/// [`Resource::MUTABLE`].
///
/// When using the derive macro, mutability can be configured via
/// `#[resource(mutable = true/false)]`.
///
/// If a resource is immutable, APIs such as `get_mut` and `fetch` cannot return
/// mutable references (they return `None`). A mutable `Query` access instead
/// returns an error, which by default may lead to a panic.
///
/// ## Dropper
///
/// [`Resource::DROPPER`] stores the function pointer for [`Drop::drop`].
///
/// [`Dropper`] extracts this pointer at compile time, so users usually do not
/// need to specify it manually.
///
/// # Safety
///
/// Implementing this trait promises that the type can be stored behind the ECS'
/// type-erased resource storage. If you override [`Self::DROPPER`], they must
/// match the implementor's actual layout and drop behavior.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a resource",
    label = "invalid resource",
    note = "Consider annotating `{Self}` with `#[derive(Resource)]`."
)]
pub trait Resource: Sized + 'static {
    const MUTABLE: bool = true;
    const DROPPER: Option<Dropper> = Dropper::of::<Self>();
}
