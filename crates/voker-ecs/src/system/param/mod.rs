//! System parameter infrastructure.
//!
//! This module defines how function parameters are transformed into runtime
//! values when systems execute.
//!
//! Conceptually, each parameter type contributes:
//! - initialization state,
//! - declared world access,
//! - a per-run fetched item.

// -----------------------------------------------------------------------------
// Modules

mod local;
mod marker;
mod resource;
mod tuples;
mod world;

// -----------------------------------------------------------------------------
// marker

pub use local::Local;
pub use marker::NonSendMarker;

// -----------------------------------------------------------------------------
// SystemParam

use super::AccessTable;
use crate::system::{SystemMeta, SystemParamError};
use crate::tick::Tick;
use crate::world::{DeferredWorld, UnsafeWorld, World};

/// Describes how a type is initialized and fetched as a system parameter.
///
/// A `SystemParam` implementation defines the full parameter lifecycle:
/// 1. build persistent per-system [`State`](SystemParam::State),
/// 2. register world access for conflict detection,
/// 3. fetch the run-local [`Item`](SystemParam::Item) from world + state,
/// 4. optionally flush deferred effects.
///
/// # Available Parameters
///
/// - [`&World`] and [`&mut World`]
/// - [`Commands`]
/// - [`Local`]
/// - [`Query`]
/// - [`Res`], [`ResRef`], [`ResMut`]
/// - [`NonSend`], [`NonSendRef`], [`NonSendMut`]
/// - [`MessageReader`], [`MessageWriter`], [`MessageMutator`]
///
/// Each parameter has a persistent [`State`](SystemParam::State) stored alongside
/// the compiled system. That state is initialized once, contributes borrow
/// information to the system access table, and is then used to fetch the concrete
/// [`Item`](SystemParam::Item) passed into the system body on each run.
///
/// The associated `Item<'world, 'state>` is itself a `SystemParam` to support
/// composable parameter wrappers and tuple flattening during compilation.
/// In practice, user code consumes the final resolved item type and does not
/// implement this recursion directly.
///
/// Built-in implementations cover individual parameters, optional parameters, and
/// tuples of parameters. Manual implementations are primarily for extending the
/// ECS runtime with new parameter kinds.
///
/// # Derive
///
/// For struct composition, prefer `#[derive(SystemParam)]`.
/// The derive combines field-level `SystemParam` implementations and enforces
/// the `'w` / `'s` lifetime shape at compile time.
///
/// ```no_run
/// use voker_ecs::borrow::Res;
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
/// }
///
/// fn tick_counter(mut param: CounterParam) {
///     *param.local += 1;
///     let _current = param.counter.0;
/// }
/// ```
///
/// # Aliasing rules
///
/// `SystemParams` must obey Rust aliasing rules. For example, `(Res<Foo>, ResMut<Foo>)` is
/// invalid and will panic at runtime.
///
/// Also note the difference between world access:
/// - `&World` represents shared access to all data in the world.
/// - `&mut World` represents exclusive access to all data in the world.
///
/// Therefore, `(&World, Res<Foo>)` is valid, while `(&World, ResMut<Foo>)` and
/// `(&mut World, Res<Foo>)` are invalid and will panic at runtime.
///
/// `Commands` is a deferred command queue and is modeled as not directly
/// accessing resources/components. Therefore, `(&mut World, Commands)` is
/// technically valid (though usually not very useful).
///
/// [`&World`]: crate::world::World
/// [`&mut World`]: crate::world::World
/// [`Commands`]: crate::command::Commands
/// [`Query`]: crate::query::Query
/// [`Res`]: crate::borrow::Res
/// [`ResRef`]: crate::borrow::ResRef
/// [`ResMut`]: crate::borrow::ResMut
/// [`NonSend`]: crate::borrow::NonSend
/// [`NonSendRef`]: crate::borrow::NonSendRef
/// [`NonSendMut`]: crate::borrow::NonSendMut
/// [`MessageReader`]: crate::message::MessageReader
/// [`MessageWriter`]: crate::message::MessageWriter
/// [`MessageMutator`]: crate::message::MessageMutator
///
/// # Safety
///
/// Implementations must report access patterns accurately from
/// [`mark_access`](SystemParam::mark_access) and must only produce items from
/// [`build_param`](SystemParam::build_param) that are valid for the supplied world and
/// ticks. Incorrect implementations can violate aliasing guarantees enforced by
/// the scheduler.
///
/// In particular, an implementation must never declare read-only access while
/// yielding mutable references (or equivalent write capability) in `build_param`.
///
/// Additional safety contract:
/// - `build_param` must treat `state` as belonging to this exact compiled system.
/// - references returned from `build_param` must not outlive the provided
///   `'w` / `'s` lifetimes.
/// - `defer` and `apply_deferred` must only operate on effects declared by
///   `DEFERRED` and the parameter's own state.
pub unsafe trait SystemParam: Sized {
    /// Persistent parameter state stored with the compiled system.
    type State: Send + Sync + 'static;

    /// Concrete parameter type produced for one system run.
    ///
    /// `'world` is tied to borrows coming from [`World`].
    /// `'state` is tied to borrows from [`State`](SystemParam::State).
    type Item<'world, 'state>: SystemParam<State = Self::State>;

    /// Whether this parameter has deferred work that requires `defer` and
    /// `apply_deferred` to run.
    const DEFERRED: bool = false;

    /// Whether this parameter is thread-affine (`NonSend`).
    const NON_SEND: bool;

    /// Whether this parameter requires exclusive world access.
    const EXCLUSIVE: bool;

    /// Initializes persistent state for this parameter when a system is built.
    fn init_state(world: &mut World) -> Self::State;

    /// Declares world/resource access used by this parameter.
    ///
    /// Returns `true` if access can be registered without conflict.
    fn mark_access(table: &mut AccessTable, state: &Self::State) -> bool;

    /// Fetches the per-run parameter value from world + state.
    ///
    /// # Safety
    ///
    /// Caller guarantees that `mark_access` was used to validate conflicts for
    /// this parameter configuration before invoking `build_param`.
    unsafe fn build_param<'w, 's>(
        world: UnsafeWorld<'w>,
        state: &'s mut Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError>;

    /// Queues deferred effects into a [`DeferredWorld`] view.
    #[inline]
    #[expect(unused_variables, reason = "default implementation")]
    fn queue_deferred(state: &mut Self::State, system_meta: &SystemMeta, world: DeferredWorld) {}

    /// Applies previously queued deferred effects to the real world.
    #[inline]
    #[expect(unused_variables, reason = "default implementation")]
    fn apply_deferred(state: &mut Self::State, system_meta: &SystemMeta, world: &mut World) {}
}
