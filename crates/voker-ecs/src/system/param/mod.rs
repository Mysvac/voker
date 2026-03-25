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
mod resource;
mod tuples;
mod world;

// -----------------------------------------------------------------------------
// marker

pub use local::Local;

// -----------------------------------------------------------------------------
// SystemParam

use super::AccessTable;
use crate::error::EcsError;
use crate::tick::Tick;
use crate::world::{UnsafeWorld, World};

/// Describes how a type is initialized and fetched as a system parameter.
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
pub unsafe trait SystemParam: Sized {
    /// Persistent parameter state stored with the compiled system.
    ///
    /// This is created once by [`SystemParam::init_state`] and reused across runs.
    type State: Send + Sync + 'static;

    /// Concrete parameter type produced for one system run.
    ///
    /// The returned item may borrow from both the world (`'world`) and the
    /// persistent state (`'state`).
    type Item<'world, 'state>: SystemParam<State = Self::State>;

    /// Whether this parameter is thread-affine (`NonSend`).
    ///
    /// If `true`, systems using this parameter must run on the main thread and
    /// cannot be sent to worker threads during scheduling.
    ///
    /// Typical examples include parameters that may touch non-thread-safe data,
    /// such as `NonSend<T>` or `&mut World`.
    const NON_SEND: bool;

    /// Whether this parameter requires exclusive world access.
    ///
    /// If `true`, systems using this parameter cannot run in parallel with any
    /// other system for that schedule step. A typical example is `&mut World`.
    const EXCLUSIVE: bool;

    /// Initializes persistent state for this parameter type.
    ///
    /// Called during system initialization, before scheduling and execution.
    fn init_state(world: &mut World) -> Self::State;

    /// Registers this parameter's access pattern in the schedule access table.
    ///
    /// Returns `false` if access registration detects a conflict.
    fn mark_access(table: &mut AccessTable, state: &Self::State) -> bool;

    /// # Safety
    /// - `world` must point to the same world used to initialize `state`.
    /// - The returned [`SystemParam::Item`] must obey the accesses previously
    ///   declared in [`SystemParam::mark_access`].
    /// - Any references or pointers embedded in the returned item must remain
    ///   valid for the full `'world` / `'state` lifetimes.
    /// - Implementations must not create aliasing violations (for example,
    ///   overlapping mutable and shared references to the same data).
    unsafe fn build_param<'w, 's>(
        world: UnsafeWorld<'w>,
        state: &'s mut Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, EcsError>;
}

/// Marker trait for parameters that only perform shared reads.
///
/// Read-only parameters can participate in systems that run concurrently with
/// other readers of the same data.
///
/// # Safety
/// The implementer must guarantee that this parameter never performs mutable
/// access to world data and never requires exclusive scheduling.
pub unsafe trait ReadOnlySystemParam: SystemParam {}
