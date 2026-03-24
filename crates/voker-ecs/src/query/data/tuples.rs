use alloc::vec::Vec;

use super::{QueryData, ReadOnlyQueryData};
use crate::archetype::Archetype;
use crate::entity::Entity;
use crate::storage::{Table, TableRow};
use crate::system::{AccessParam, FilterParamBuilder};
use crate::tick::Tick;
use crate::world::{UnsafeWorld, World};

macro_rules! impl_tuple {
    (0: []) => {
        unsafe impl ReadOnlyQueryData for () {}

        unsafe impl QueryData for () {
            type State = ();
            type Cache<'world> = ();
            type Item<'world> = ();

            const COMPONENTS_ARE_DENSE: bool = true;

            fn build_state(_world: &mut World) -> Self::State {}

            unsafe fn build_cache<'w>(
                _state: &Self::State,
                _world: UnsafeWorld<'w>,
                _last_run: Tick,
                _this_run: Tick,
            ) -> Self::Cache<'w> {}

            fn build_filter(_state: &Self::State, _out: &mut Vec<FilterParamBuilder>) {}
            fn build_access(_state: &Self::State, _out: &mut AccessParam) -> bool { true }

            unsafe fn set_for_arche<'w>(
                _state: &Self::State,
                _cache: &mut Self::Cache<'w>,
                _arche: &'w Archetype,
                _table: &'w Table,
            ) {}

            unsafe fn set_for_table<'w>(
                _state: &Self::State,
                _cache: &mut Self::Cache<'w>,
                _table: &'w Table,
            ) {}

            unsafe fn fetch<'w>(
                _state: &Self::State,
                _cache: &mut Self::Cache<'w>,
                _entity: Entity,
                _table_row: TableRow,
            ) -> Option<Self::Item<'w>> {
                Some(())
            }
        }
    };
    (1 : [ $index:tt : $name:ident ]) => {
        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 12 items long.")]
        unsafe impl<$name: ReadOnlyQueryData> ReadOnlyQueryData for ($name,) {}

        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 12 items long.")]
        unsafe impl<$name: QueryData> QueryData for ($name,) {
            type State = <$name>::State;
            type Cache<'world> = <$name>::Cache<'world>;
            type Item<'world> = ( <$name>::Item<'world>, );

            const COMPONENTS_ARE_DENSE: bool = <$name>::COMPONENTS_ARE_DENSE;

            fn build_state(world: &mut World) -> Self::State {
                <$name>::build_state(world)
            }

            unsafe fn build_cache<'w>(
                state: &Self::State,
                world: UnsafeWorld<'w>,
                last_run: Tick,
                this_run: Tick,
            ) -> Self::Cache<'w> {
                unsafe { <$name>::build_cache(state, world, last_run, this_run) }
            }

            fn build_filter(state: &Self::State, out: &mut Vec<FilterParamBuilder>) {
                <$name>::build_filter(state, out);
            }

            fn build_access(state: &Self::State, out: &mut AccessParam) -> bool {
                <$name>::build_access(state, out)
            }

            unsafe fn set_for_arche<'w>(
                state: &Self::State,
                cache: &mut Self::Cache<'w>,
                arche: &'w Archetype,
                table: &'w Table,
            ) {
                unsafe { <$name>::set_for_arche(state, cache, arche, table); }
            }

            unsafe fn set_for_table<'w>(
                state: &Self::State,
                cache: &mut Self::Cache<'w>,
                table: &'w Table,
            ) {
                unsafe { <$name>::set_for_table(state, cache, table); }
            }

            unsafe fn fetch<'w>(
                state: &Self::State,
                cache: &mut Self::Cache<'w>,
                entity: Entity,
                table_row: TableRow,
            ) -> Option<Self::Item<'w>> {
                unsafe { Some(( <$name>::fetch(state, cache, entity, table_row)?, )) }
            }
        }
    };
    ($num:literal : [$($index:tt : $name:ident),*]) => {
        #[cfg_attr(docsrs, doc(hidden))]
        unsafe impl<$($name: ReadOnlyQueryData),*> ReadOnlyQueryData for ($($name),*) {}

        #[cfg_attr(docsrs, doc(hidden))]
        unsafe impl<$($name: QueryData),*> QueryData for ($($name),*) {
            type State = ( $( <$name>::State ),* );
            type Cache<'world> = ( $( <$name>::Cache<'world> ),* );
            type Item<'world> = ( $( <$name>::Item<'world> ),* );

            const COMPONENTS_ARE_DENSE: bool = { true $( && <$name>::COMPONENTS_ARE_DENSE )* };

            fn build_state(world: &mut World) -> Self::State {
                ( $( <$name>::build_state(world), )* )
            }

            unsafe fn build_cache<'w>(
                state: &Self::State,
                world: UnsafeWorld<'w>,
                last_run: Tick,
                this_run: Tick,
            ) -> Self::Cache<'w> {
                unsafe {
                    ( $( <$name>::build_cache(&state.$index, world, last_run, this_run), )* )
                }
            }

            fn build_filter(state: &Self::State, out: &mut Vec<FilterParamBuilder>) {
                $( <$name>::build_filter(&state.$index, out); )*
            }

            fn build_access(state: &Self::State, out: &mut AccessParam) -> bool {
                true $( && <$name>::build_access(&state.$index, out) )*
            }

            unsafe fn set_for_arche<'w>(
                state: &Self::State,
                cache: &mut Self::Cache<'w>,
                arche: &'w Archetype,
                table: &'w Table,
            ) {
                unsafe {
                    $( <$name>::set_for_arche(&state.$index, &mut cache.$index, arche, table); )*
                }
            }

            unsafe fn set_for_table<'w>(
                state: &Self::State,
                cache: &mut Self::Cache<'w>,
                table: &'w Table,
            ) {
                unsafe {
                    $( <$name>::set_for_table(&state.$index, &mut cache.$index, table); )*
                }
            }

            unsafe fn fetch<'w>(
                state: &Self::State,
                cache: &mut Self::Cache<'w>,
                entity: Entity,
                table_row: TableRow,
            ) -> Option<Self::Item<'w>> {
                unsafe {
                    Some(( $( <$name>::fetch(&state.$index, &mut cache.$index, entity, table_row)?, )* ))
                }
            }
        }
    };
}

voker_utils::range_invoke!(impl_tuple, 12);
