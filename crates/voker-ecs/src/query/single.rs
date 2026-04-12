use alloc::string::ToString;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

use crate::error::Severity;
use crate::query::{QueryData, QueryFilter, QueryIter, QuerySingleError, QueryState};
use crate::system::{AccessTable, SystemParam, SystemParamError};
use crate::tick::Tick;
use crate::world::{UnsafeWorld, World};

/// A system parameter that guarantees exactly one matching query item.
///
/// This wraps `Query` single-target access and fails parameter construction
/// with [`SystemParamError`] when the query has zero or multiple matches.
///
/// Use this when your system semantics require one and only one target.
///
/// # Example
///
/// ```ignore
/// fn update_player(mut player: Single<&mut Player>) {
///     player.health = player.health.saturating_sub(1);
/// }
/// ```
#[repr(transparent)]
pub struct Single<'world, D: QueryData, F: QueryFilter = ()> {
    pub(super) item: D::Item<'world>,
    pub(super) _marker: PhantomData<F>,
}

// -----------------------------------------------------------------------------
// SystemParam

impl<'w, D: QueryData, F: QueryFilter> Single<'w, D, F> {
    pub unsafe fn new<'s>(
        world: UnsafeWorld<'w>,
        state: &'s QueryState<D, F>,
        last_run: Tick,
        this_run: Tick,
    ) -> Result<Self, QuerySingleError> {
        let mut iter = unsafe { QueryIter::new(world, state, last_run, this_run) };

        let Some(item) = iter.next() else {
            return Err(QuerySingleError::NoEntities);
        };

        if iter.next().is_some() {
            return Err(QuerySingleError::MultipleEntities);
        }

        Ok(Single {
            item,
            _marker: PhantomData,
        })
    }
}

unsafe impl<D: QueryData + 'static, F: QueryFilter + 'static> SystemParam for Single<'_, D, F> {
    type State = QueryState<D, F>;
    type Item<'world, 'state> = Single<'world, D, F>;

    const NON_SEND: bool = false;
    const EXCLUSIVE: bool = false;

    fn init_state(world: &mut World) -> Self::State {
        QueryState::build(world)
    }

    fn mark_access(table: &mut AccessTable, state: &Self::State) -> bool {
        state.mark_assess(table)
    }

    unsafe fn build_param<'w, 's>(
        world: UnsafeWorld<'w>,
        state: &'s mut Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError> {
        state.update(unsafe { world.read_only() });

        match unsafe { Single::new(world, state, last_run, this_run) } {
            Ok(ret) => Ok(ret),
            Err(e) => {
                voker_utils::cold_path();
                let error = SystemParamError::new::<Self>()
                    .with_severity(Severity::Warning)
                    .with_info(ToString::to_string(&e));
                Err(error)
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Basic

impl<'w, D: QueryData, F: QueryFilter> Single<'w, D, F> {
    /// Consumes this wrapper and returns the inner query item.
    #[inline(always)]
    pub fn into_inner(self) -> D::Item<'w> {
        self.item
    }
}

impl<'world, D: QueryData, F: QueryFilter> Deref for Single<'world, D, F> {
    type Target = D::Item<'world>;

    fn deref(&self) -> &Self::Target {
        &self.item
    }
}

impl<'world, D: QueryData, F: QueryFilter> DerefMut for Single<'world, D, F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.item
    }
}
