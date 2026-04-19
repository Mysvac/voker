use alloc::boxed::Box;
use core::panic::{RefUnwindSafe, UnwindSafe};
use core::{fmt::Debug, marker::PhantomData};

use crate::define_label;
use crate::label::Interned;
use crate::system::{AccessTable, IntoSystem, System, SystemError, SystemFlags, SystemId};
use crate::world::{DeferredWorld, UnsafeWorld};
use crate::{tick::Tick, utils::DebugName, world::World};

// -----------------------------------------------------------------------------
// SystemSet

define_label!(
    /// A strongly-typed class of labels used to identify a `SystemSet`.
    ///
    /// System-set labels and boundary marker systems.
    ///
    /// `SystemSet` in voker is implemented as a graph-level boundary mechanism.
    /// Each set provides two no-op marker systems:
    /// - begin marker: [`SystemSetBegin`]
    /// - end marker: [`SystemSetEnd`]
    ///
    /// When a system is inserted into a set, the schedule inserts condition edges:
    /// - `begin -> system`
    /// - `system -> end`
    /// - `begin -> end` (keeps an explicit empty-set boundary)
    ///
    /// This means set membership is enforced by existing schedule dependency and
    /// condition graphs, without introducing a separate set executor.
    ///
    /// The const generic `TAG` is used to distinguish markers under the same set
    /// type. Derive macros use:
    /// - `TAG = 0` for unit structs
    /// - variant index for fieldless enums
    /// so different enum variants produce distinct begin/end marker ids.
    ///
    /// Most users should use `#[derive(SystemSet)]`.
    #[diagnostic::on_unimplemented(
        note = "consider annotating `{Self}` with `#[derive(SystemSet)]`"
    )]
    SystemSet,
    SYSTEM_SET_INTERNER,
    extra_methods: {
        /// Returns the begin-boundary marker for this set.
        ///
        /// The returned system should be a no-op with a stable [`SystemId`].
        fn begin(&self) -> Box<dyn System<Input = (), Output = ()>>;

        /// Returns the end-boundary marker for this set.
        ///
        /// The returned system should be a no-op with a stable [`SystemId`].
        fn end(&self) -> Box<dyn System<Input = (), Output = ()>>;
    },
    extra_methods_impl: {
        fn begin(&self) -> Box<dyn System<Input = (), Output = ()>> {
            (**self).begin()
        }

        fn end(&self) -> Box<dyn System<Input = (), Output = ()>> {
            (**self).end()
        }
    }
);

/// A shorthand for interned `SystemSet` labels.
pub type InternedSystemSet = Interned<dyn SystemSet>;

// -----------------------------------------------------------------------------
// SystemSetBegin & SystemSetEnd

/// Begin-boundary signal for a specific `SystemSet` marker identity.
///
/// `ID` identifies the set type, while `TAG` disambiguates variants of the
/// same set type (for example enum variants in derive-generated impls).
pub struct SystemSetBegin<ID, const TAG: usize>(PhantomData<ID>, PhantomData<[(); TAG]>);

/// End-boundary signal for a specific `SystemSet` marker identity.
///
/// See [`SystemSetBegin`] for `ID`/`TAG` semantics.
pub struct SystemSetEnd<ID, const TAG: usize>(PhantomData<ID>, PhantomData<[(); TAG]>);

macro_rules! impl_signal {
    ($name:ident) => {
        impl<ID, const TAG: usize> Default for $name<ID, TAG> {
            fn default() -> Self {
                Self(PhantomData, PhantomData)
            }
        }

        impl<ID, const TAG: usize> $name<ID, TAG> {
            pub const fn new() -> Self {
                Self(PhantomData, PhantomData)
            }
        }

        unsafe impl<ID, const TAG: usize> Send for $name<ID, TAG> {}
        unsafe impl<ID, const TAG: usize> Sync for $name<ID, TAG> {}
        impl<ID, const TAG: usize> UnwindSafe for $name<ID, TAG> {}
        impl<ID, const TAG: usize> RefUnwindSafe for $name<ID, TAG> {}

        impl<ID, const TAG: usize> Debug for $name<ID, TAG> {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_tuple("SystemSetBegin")
                    .field(&DebugName::type_name::<ID>())
                    .field(&TAG)
                    .finish()
            }
        }

        impl<ID, const TAG: usize> Copy for $name<ID, TAG> {}

        impl<ID, const TAG: usize> Clone for $name<ID, TAG> {
            fn clone(&self) -> Self {
                *self
            }
        }

        impl<ID: 'static, const TAG: usize> System for $name<ID, TAG> {
            type Input = ();
            type Output = ();

            fn id(&self) -> SystemId {
                SystemId::of::<Self>()
            }

            fn flags(&self) -> SystemFlags {
                SystemFlags::NO_OP
            }

            fn last_run(&self) -> Tick {
                Tick::new(0)
            }

            fn set_last_run(&mut self, _: Tick) {}

            fn initialize(&mut self, _: &mut World) -> AccessTable {
                AccessTable::new()
            }

            unsafe fn run_raw(
                &mut self,
                _: (),
                _: UnsafeWorld<'_>,
            ) -> Result<Self::Output, SystemError> {
                Ok(())
            }

            fn queue_deferred(&mut self, _: DeferredWorld) {}

            fn apply_deferred(&mut self, _: &mut World) {}

            fn run(&mut self, _: (), _: &mut World) -> Result<Self::Output, SystemError> {
                Ok(())
            }

            fn is_no_op(&self) -> bool {
                true
            }

            fn is_deferred(&self) -> bool {
                false
            }

            fn is_non_send(&self) -> bool {
                false
            }

            fn is_exclusive(&self) -> bool {
                false
            }
        }
    };
}

impl_signal!(SystemSetBegin);
impl_signal!(SystemSetEnd);

// -----------------------------------------------------------------------------
// AnonymousSystemSet

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnonymousSystemSet;

impl SystemSet for AnonymousSystemSet {
    fn begin(&self) -> Box<dyn System<Input = (), Output = ()>> {
        Box::new(IntoSystem::into_system(<SystemSetBegin<Self, 0>>::new()))
    }

    fn end(&self) -> Box<dyn System<Input = (), Output = ()>> {
        Box::new(IntoSystem::into_system(<SystemSetEnd<Self, 0>>::new()))
    }

    fn dyn_clone(&self) -> Box<dyn SystemSet> {
        Box::new(AnonymousSystemSet)
    }
}
