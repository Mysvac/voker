#![expect(clippy::module_inception, reason = "For better structure.")]

use core::fmt::Debug;
use core::marker::PhantomData;

use crate::system::{AccessTable, SystemError, SystemFlags, SystemId};
use crate::tick::Tick;
use crate::world::{UnsafeWorld, World};

use super::SystemInput;

// -----------------------------------------------------------------------------
// System

/// Core trait defining a runnable unit of logic in the ECS.
///
/// A `System` encapsulates executable logic that can operate on the ECS world,
/// with clearly defined input and output types. Systems are the fundamental
/// building blocks for game logic, simulation steps, and reactive behaviors.
///
/// Any Rust function with a compatible signature can be used as a system, for example:
///
/// ```ignore
/// fn system_a(query: Query<&Name, Changed<Health>>) {
///     /* do something */
/// }
/// ```
///
/// # Parallelism
///
/// At the moment, systems run through [`Schedule`](crate::schedule::Schedule),
/// which builds an execution graph from system parameters to maximize parallel
/// execution.
///
/// Two systems can run in parallel when their accesses do not conflict under
/// read/write exclusion rules.
///
/// For example, these two systems can run in parallel because they access
/// different data:
///
/// ```ignore
/// fn system_a(query: Query<&Bar>, res: Res<Baz>) { }
/// fn system_b(query: Query<&mut Foo>) { }
/// ```
///
/// For `Query`, systems can also run in parallel if their filter constraints
/// guarantee they never touch the same data:
///
/// ```ignore
/// fn system_a(query: Query<&mut Foo, With<Bar>) { }
/// fn system_b(query: Query<&mut Foo, Without<Bar>>) { }
/// ```
///
/// ## Special Cases
///
/// There are two special categories of systems.
///
/// A system that accesses `NonSend` data cannot be moved across threads,
/// so it must be scheduled on the main thread:
///
/// ```ignore
/// fn system_a(foo: NonSend<Foo>) {
///     /* do something */
/// }
/// ```
///
/// A system that takes `&mut World` is fully exclusive and cannot run in
/// parallel with any other system:
///
/// ```ignore
/// fn system_a(world: &mut World) {
///     /* do something */
/// }
/// ```
///
/// Fully exclusive systems can limit parallel performance. For workloads such
/// as spawning/despawning entities that require world mutation, prefer
/// [`Commands`](crate::command::Commands) as a deferred alternative:
///
/// ```ignore
/// fn system_a(mut commands: Commands) {
///     /* do something */
/// }
/// ```
///
/// Commands submitted through `Commands` are not executed immediately. They are
/// pushed into the world's deferred command queue, which is thread-safe.
/// Therefore, `Commands` does not count as direct component/resource access and
/// does not reduce system parallelism.
///
/// # Access Declaration Contract
///
/// A system must follow this contract:
/// 1. [`initialize`](System::initialize) returns an [`AccessTable`] that fully
///    describes all accesses the system may perform,
/// 2. [`run`](System::run) must not perform accesses outside that declaration.
///
/// Violating this contract can cause unsound parallel execution.
///
/// # Debugging Missing Parallelism
///
/// If two systems unexpectedly serialize, check in order:
/// 1. whether either system is `EXCLUSIVE` (`&mut World`),
/// 2. whether a shared resource is written by one side (`ResMut` / `NonSendMut`),
/// 3. whether query filters are actually disjoint,
/// 4. whether explicit schedule ordering edges force sequencing.
#[diagnostic::on_unimplemented(message = "`{Self}` is not a system", label = "invalid system")]
pub trait System: Send + Sync + 'static {
    /// The system's input.
    type Input: SystemInput;
    /// The system's output.
    type Output;

    /// Returns the system's name for debugging and identification purposes.
    fn id(&self) -> SystemId;

    /// Returns the system's behavioral flags.
    ///
    /// Flags control how the system is scheduled and executed:
    /// - `NON_SEND`: System cannot be moved between threads
    /// - `EXCLUSIVE`: System requires exclusive world access
    fn flags(&self) -> SystemFlags;

    /// Gets the tick when this system last completed execution.
    fn last_run(&self) -> Tick;

    /// Sets the tick when this system last completed execution.
    fn set_last_run(&mut self, last_run: Tick);

    /// Initializes the system, registering any required components or resources.
    ///
    /// The implementer must allow for repeated initialization.
    fn initialize(&mut self, world: &mut World) -> AccessTable;

    /// Executes the system's logic against the provided world, without apply_deferred.
    ///
    /// # Safety
    ///
    /// - The caller must ensure that the world's access patterns do not conflict
    ///   with other systems running concurrently.
    /// - The implementation must respect the access patterns declared in
    ///   `initialize` and not access components/resources outside those patterns.
    /// - For `NON_SEND` systems, the caller must ensure execution occurs on the
    ///   same thread where the system was created.
    /// - For `EXCLUSIVE` systems, the caller must ensure exclusive world access.
    unsafe fn run_raw(
        &mut self,
        input: <Self::Input as SystemInput>::Data<'_>,
        world: UnsafeWorld<'_>,
    ) -> Result<Self::Output, SystemError>;

    fn run(
        &mut self,
        input: <Self::Input as SystemInput>::Data<'_>,
        world: &mut World,
    ) -> Result<Self::Output, SystemError> {
        let result = unsafe { self.run_raw(input, world.into())? };
        self.apply_deferred(world);
        Ok(result)
    }

    /// Applies queued deferred mutations to `World`.
    ///
    /// The scheduler calls this only when [`System::is_deferred`] is `true`.
    fn apply_deferred(&mut self, world: &mut World);

    /// Returns `true` if this system is no-op.
    ///
    /// This is currently only used for `apply_defered`.
    /// It does not perform any additional operations and can be optimized when repeated.
    #[inline]
    fn is_no_op(&self) -> bool {
        self.flags().intersects(SystemFlags::NO_OP)
    }

    /// Returns `true` if this system is marked as `DEFERRED`.
    #[inline]
    fn is_deferred(&self) -> bool {
        self.flags().intersects(SystemFlags::DEFERRED)
    }

    /// Returns `true` if this system is marked as `NON_SEND`.
    #[inline]
    fn is_non_send(&self) -> bool {
        self.flags().intersects(SystemFlags::NON_SEND)
    }

    /// Returns `true` if this system is marked as `EXCLUSIVE`.
    #[inline]
    fn is_exclusive(&self) -> bool {
        self.flags().intersects(SystemFlags::EXCLUSIVE)
    }
}

impl<I, O> Debug for dyn System<Input = I, Output = O>
where
    I: SystemInput + 'static,
    O: 'static,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("System")
            .field("id", &self.id())
            .field("deferred", &self.is_deferred())
            .field("non_send", &self.is_non_send())
            .field("exclusive", &self.is_exclusive())
            .finish_non_exhaustive()
    }
}

// -----------------------------------------------------------------------------
// IntoSystem

/// Trait for converting a value into a [`System`].
///
/// This trait enables ergonomic system construction from closures, functions,
/// and combinators. It serves as the entry point for creating systems that
/// can be scheduled and executed by the ECS.
///
/// # Combinators
///
/// `IntoSystem` provides several combinator methods for system composition:
///
/// - [`pipe`](IntoSystem::pipe): Chain two systems, feeding output of first as input to second
/// - [`map`](IntoSystem::map): Transform system output using a function
/// - [`mark`](IntoSystem::mark): Conditionally run the system based on another system's output
pub trait IntoSystem<I: SystemInput, O, M>: Sized + 'static {
    type System: System<Input = I, Output = O>;

    fn into_system(this: Self) -> Self::System;

    fn system_id(&self) -> SystemId {
        SystemId::of::<Self>()
    }

    fn with_input(self, input: I::Data<'static>) -> IntoWithInputSystem<Self, I>
    where
        I::Data<'static>: Clone,
    {
        IntoWithInputSystem { s: self, i: input }
    }

    fn pipe<B, BI, BO, MB>(self, other: B) -> IntoPipeSystem<Self, B>
    where
        O: 'static,
        B: IntoSystem<BI, BO, MB>,
        for<'a> BI: SystemInput<Data<'a> = O>,
    {
        IntoPipeSystem { a: self, b: other }
    }

    fn map<F, FO>(self, func: F) -> IntoMapSystem<Self, F>
    where
        F: FnMut(O) -> FO + Sync + Send + 'static,
    {
        IntoMapSystem { s: self, f: func }
    }

    fn mark<Marker: 'static>(self) -> IntoMarkSystem<Self, Marker> {
        IntoMarkSystem {
            s: self,
            _marker: PhantomData,
        }
    }
}

// -----------------------------------------------------------------------------
// System itself

impl<T: System> IntoSystem<T::Input, T::Output, ()> for T {
    type System = T;

    fn into_system(this: Self) -> Self {
        this
    }

    fn system_id(&self) -> SystemId {
        <T as System>::id(self)
    }
}

// -----------------------------------------------------------------------------
// IntoWithInputSystem

pub struct WithInputSystemMarker;

pub struct IntoWithInputSystem<S, I: SystemInput> {
    s: S,
    i: I::Data<'static>,
}

pub struct WithInputSystem<S, I: SystemInput> {
    id: SystemId,
    s: S,
    i: I::Data<'static>,
}

#[rustfmt::skip]
impl<I, O, S, M> IntoSystem<(), O, (WithInputSystemMarker, (M, fn(I) -> O))>
    for IntoWithInputSystem<S, I>
where
    I: SystemInput + 'static,
    I::Data<'static>: Clone + Send + Sync,
    S: IntoSystem<I, O, M>,
    M: 'static,
{
    type System = WithInputSystem<S::System, I>;

    fn into_system(this: Self) -> Self::System {
        WithInputSystem {
            id: Self::system_id(&this),
            s: IntoSystem::into_system(this.s),
            i: this.i,
        }
    }

    fn system_id(&self) -> SystemId {
        struct WithInput<T>(PhantomData<T>);
        SystemId::of::<(WithInput<I>, S)>()
    }
}

impl<I, O, S> System for WithInputSystem<S, I>
where
    I: SystemInput + 'static,
    I::Data<'static>: Clone + Send + Sync,
    S: System<Input = I, Output = O>,
{
    type Input = ();
    type Output = O;

    fn id(&self) -> SystemId {
        self.id
    }

    fn flags(&self) -> SystemFlags {
        self.s.flags()
    }

    fn last_run(&self) -> Tick {
        self.s.last_run()
    }

    fn set_last_run(&mut self, last_run: Tick) {
        self.s.set_last_run(last_run);
    }

    fn initialize(&mut self, world: &mut World) -> AccessTable {
        self.s.initialize(world)
    }

    unsafe fn run_raw(
        &mut self,
        _input: (),
        world: UnsafeWorld<'_>,
    ) -> Result<Self::Output, SystemError> {
        unsafe { self.s.run_raw(self.i.clone(), world) }
    }

    fn apply_deferred(&mut self, world: &mut World) {
        self.s.apply_deferred(world);
    }
}

// -----------------------------------------------------------------------------
// IntoPipeSystem

pub struct PipeSystemMarker;

#[derive(Clone, Copy)]
pub struct IntoPipeSystem<A, B> {
    a: A,
    b: B,
}

pub struct PipeSystem<A, B> {
    id: SystemId,
    a: A,
    b: B,
}

#[rustfmt::skip]
impl<AI, AO, BI, BO, A, B, MA, MB>
    IntoSystem<AI, BO, (PipeSystemMarker, (MA, MB, fn(AI) -> AO, fn(BI) -> BO), (A, B))>
    for IntoPipeSystem<A, B>
where
    AI: SystemInput,
    for<'a> BI: SystemInput<Data<'a> = AO>,
    A: IntoSystem<AI, AO, MA>,
    B: IntoSystem<BI, BO, MB>,
{
    type System = PipeSystem<A::System, B::System>;

    fn into_system(this: Self) -> Self::System {
        PipeSystem {
            id: Self::system_id(&this),
            a: IntoSystem::into_system(this.a),
            b: IntoSystem::into_system(this.b),
        }
    }

    fn system_id(&self) -> SystemId {
        struct Pipe;
        SystemId::of::<(A, Pipe, B)>()
    }
}

impl<AI, AO, BI, BO, A, B> System for PipeSystem<A, B>
where
    AI: SystemInput,
    for<'a> BI: SystemInput<Data<'a> = AO>,
    A: System<Input = AI, Output = AO>,
    B: System<Input = BI, Output = BO>,
{
    type Input = AI;
    type Output = BO;

    fn id(&self) -> SystemId {
        self.id
    }

    fn flags(&self) -> SystemFlags {
        self.a.flags().union(self.b.flags())
    }

    fn last_run(&self) -> Tick {
        self.a.last_run()
    }

    fn set_last_run(&mut self, last_run: Tick) {
        self.a.set_last_run(last_run);
        self.b.set_last_run(last_run);
    }

    fn initialize(&mut self, world: &mut World) -> AccessTable {
        self.a.initialize(world).merge(self.b.initialize(world))
    }

    unsafe fn run_raw(
        &mut self,
        input: <Self::Input as SystemInput>::Data<'_>,
        world: UnsafeWorld<'_>,
    ) -> Result<Self::Output, SystemError> {
        let data = unsafe { self.a.run_raw(input, world)? };
        unsafe { self.b.run_raw(data, world) }
    }

    fn apply_deferred(&mut self, world: &mut World) {
        self.a.apply_deferred(world);
        self.b.apply_deferred(world);
    }
}

// -----------------------------------------------------------------------------
// IntoMapSystem

pub struct MapSystemMarker;

#[derive(Clone, Copy)]
pub struct IntoMapSystem<S, F> {
    s: S,
    f: F,
}

pub struct MapSystem<S, F> {
    id: SystemId,
    s: S,
    f: F,
}

#[rustfmt::skip]
impl<I, O, FO, S, F, M>
    IntoSystem<I, FO, (MapSystemMarker, (M, fn(I) -> O, fn(O) -> FO), (S, F))>
    for IntoMapSystem<S, F>
where
    I: SystemInput,
    S: IntoSystem<I, O, M>,
    F: FnMut(O) -> FO + Sync + Send + 'static,
{
    type System = MapSystem<S::System, F>;

    fn into_system(this: Self) -> Self::System {
        MapSystem {
            id: Self::system_id(&this),
            s: IntoSystem::into_system(this.s),
            f: this.f,
        }
    }

    fn system_id(&self) -> SystemId {
        struct Map;
        SystemId::of::<(S, Map, F)>()
    }
}

impl<I, O, FO, S, F> System for MapSystem<S, F>
where
    I: SystemInput,
    S: System<Input = I, Output = O>,
    F: FnMut(O) -> FO + Sync + Send + 'static,
{
    type Input = I;
    type Output = FO;

    fn id(&self) -> SystemId {
        self.id
    }

    fn flags(&self) -> SystemFlags {
        self.s.flags()
    }

    fn last_run(&self) -> Tick {
        self.s.last_run()
    }

    fn set_last_run(&mut self, last_run: Tick) {
        self.s.set_last_run(last_run);
    }

    fn initialize(&mut self, world: &mut World) -> AccessTable {
        self.s.initialize(world)
    }

    unsafe fn run_raw(
        &mut self,
        input: <Self::Input as SystemInput>::Data<'_>,
        world: UnsafeWorld<'_>,
    ) -> Result<Self::Output, SystemError> {
        let data = unsafe { self.s.run_raw(input, world)? };
        Ok((self.f)(data))
    }

    fn apply_deferred(&mut self, world: &mut World) {
        self.s.apply_deferred(world);
    }
}

// -----------------------------------------------------------------------------
// IntoMarkSystem

pub struct MarkSystemMarker;

impl<S: Clone, M> Clone for IntoMarkSystem<S, M> {
    fn clone(&self) -> Self {
        Self {
            s: self.s.clone(),
            _marker: self._marker,
        }
    }
}

impl<S: Copy, M> Copy for IntoMarkSystem<S, M> {}

pub struct IntoMarkSystem<S, M> {
    s: S,
    _marker: PhantomData<M>,
}

pub struct MarkSystem<S, M> {
    id: SystemId,
    s: S,
    _marker: PhantomData<M>,
}

unsafe impl<S: Send, M> Send for IntoMarkSystem<S, M> {}
unsafe impl<S: Send, M> Send for MarkSystem<S, M> {}
unsafe impl<S: Sync, M> Sync for IntoMarkSystem<S, M> {}
unsafe impl<S: Sync, M> Sync for MarkSystem<S, M> {}

#[rustfmt::skip]
impl<I, O, S, M1, M2>
    IntoSystem<I, O, (MarkSystemMarker, (M2, M1, fn(I) -> O), (S, M2))>
    for IntoMarkSystem<S, M2>
where
    I: SystemInput,
    S: IntoSystem<I, O, M1>,
    M2: 'static,
{
    type System = MarkSystem<S::System, M2>;

    fn into_system(this: Self) -> Self::System {
        MarkSystem {
            id: Self::system_id(&this),
            s: IntoSystem::into_system(this.s),
            _marker: PhantomData,
        }
    }

    fn system_id(&self) -> SystemId {
        struct Marker<T>(PhantomData<T>);
        SystemId::of::<(Marker<M2>, S)>()
    }
}

impl<I, O, S, M> System for MarkSystem<S, M>
where
    I: SystemInput,
    S: System<Input = I, Output = O>,
    M: 'static,
{
    type Input = I;
    type Output = O;

    fn id(&self) -> SystemId {
        self.id
    }

    fn flags(&self) -> SystemFlags {
        self.s.flags()
    }

    fn last_run(&self) -> Tick {
        self.s.last_run()
    }

    fn set_last_run(&mut self, last_run: Tick) {
        self.s.set_last_run(last_run);
    }

    fn initialize(&mut self, world: &mut World) -> AccessTable {
        self.s.initialize(world)
    }

    unsafe fn run_raw(
        &mut self,
        input: <Self::Input as SystemInput>::Data<'_>,
        world: UnsafeWorld<'_>,
    ) -> Result<Self::Output, SystemError> {
        unsafe { self.s.run_raw(input, world) }
    }

    fn apply_deferred(&mut self, world: &mut World) {
        self.s.apply_deferred(world);
    }
}
