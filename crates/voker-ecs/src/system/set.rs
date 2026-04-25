use alloc::boxed::Box;
use core::fmt::Debug;
use core::marker::PhantomData;
use core::panic::{RefUnwindSafe, UnwindSafe};

use voker_utils::debug::DebugName;

use crate::define_label;
use crate::label::Interned;
use crate::system::{AccessTable, System, SystemError, SystemFlags, SystemId};
use crate::tick::Tick;
use crate::world::{DeferredWorld, UnsafeWorld, World};

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
        // Box::new(SystemSetBegin::<Self>(PhantomData, self.intern()))

        /// Returns the end-boundary marker for this set.
        ///
        /// The returned system should be a no-op with a stable [`SystemId`].
        fn end(&self) -> Box<dyn System<Input = (), Output = ()>>;
        // Box::new(SystemSetEnd::<Self>(PhantomData, self.intern()))
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
pub struct SystemSetBegin<Set>(PhantomData<Set>, InternedSystemSet);

/// End-boundary signal for a specific `SystemSet` marker identity.
///
/// See [`SystemSetBegin`] for `ID`/`TAG` semantics.
pub struct SystemSetEnd<Set>(PhantomData<Set>, InternedSystemSet);

macro_rules! impl_signal {
    ($name:ident) => {
        unsafe impl<ID> Send for $name<ID> {}
        unsafe impl<ID> Sync for $name<ID> {}
        impl<ID> UnwindSafe for $name<ID> {}
        impl<ID> RefUnwindSafe for $name<ID> {}

        impl<ID> $name<ID> {
            #[inline]
            pub fn new(set: InternedSystemSet) -> Self {
                Self(PhantomData, set)
            }
        }

        impl<ID> Debug for $name<ID> {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_tuple("SystemSetBegin")
                    .field(&DebugName::type_name::<ID>())
                    .field(&self.1)
                    .finish()
            }
        }

        impl<ID> Copy for $name<ID> {}

        impl<ID> Clone for $name<ID> {
            fn clone(&self) -> Self {
                *self
            }
        }

        impl<ID: 'static> System for $name<ID> {
            type Input = ();
            type Output = ();

            fn id(&self) -> SystemId {
                SystemId::of::<Self>().with_system_set(self.1)
            }

            fn flags(&self) -> SystemFlags {
                SystemFlags::NO_OP
            }

            fn last_run(&self) -> Tick {
                Tick::new(0)
            }

            fn set_last_run(&mut self, _: Tick) {}

            fn check_ticks(&mut self, _: Tick) {}

            fn initialize(&mut self, _: &mut World) -> AccessTable {
                AccessTable::new()
            }

            fn system_set(&self) -> InternedSystemSet {
                self.1
            }

            fn set_system_set(&mut self, _: InternedSystemSet) {}

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
                true // NOOP allows it to be quickly skipped.
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

impl SystemSet for () {
    fn begin(&self) -> Box<dyn System<Input = (), Output = ()>> {
        Box::new(SystemSetBegin::<Self>::new(self.intern()))
    }

    fn end(&self) -> Box<dyn System<Input = (), Output = ()>> {
        Box::new(SystemSetEnd::<Self>::new(self.intern()))
    }

    fn dyn_clone(&self) -> Box<dyn SystemSet> {
        Box::new(())
    }
}
