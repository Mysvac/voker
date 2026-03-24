use alloc::vec::Vec;

use super::QueryFilter;
use crate::archetype::Archetype;
use crate::entity::Entity;
use crate::storage::{Table, TableRow};
use crate::system::{AccessParam, FilterParamBuilder};
use crate::tick::Tick;
use crate::world::{UnsafeWorld, World};

// -----------------------------------------------------------------------------
// InAnd

#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be used in `And<..>`",
    label = "Expected a tuple of 1-12 elements, each implementing `QueryFilter`",
    note = "If there are more than 12 elements, nesting can be used."
)]
pub trait InAnd {}

// -----------------------------------------------------------------------------
// With

pub struct And<T: InAnd>(T);

// -----------------------------------------------------------------------------
// With for Tuple

macro_rules! impl_tuple {
    (0 : []) => {};
    (1 : [ $index:tt : $name:ident ]) => {
        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 12 items long.")]
        impl<$name: QueryFilter> InAnd for ($name,) {}

        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 12 items long.")]
        unsafe impl<$name: QueryFilter> QueryFilter for And<($name,)> {
            type State = <$name>::State;
            type Cache<'world> = <$name>::Cache<'world>;

            const COMPONENTS_ARE_DENSE: bool = <$name>::COMPONENTS_ARE_DENSE;
            const ENABLE_ENTITY_FILTER: bool = <$name>::ENABLE_ENTITY_FILTER;

            fn build_state(
                world: &mut World,
            ) -> Self::State {
                <$name>::build_state(world)
            }

            unsafe fn build_cache<'w>(
                state: &Self::State,
                world: UnsafeWorld<'w>,
                last_run: Tick,
                this_run: Tick,
            ) -> Self::Cache<'w> {
                unsafe {
                    <$name>::build_cache(state, world, last_run, this_run)
                }
            }

            fn build_filter(
                state: &Self::State,
                outer: &mut Vec<FilterParamBuilder>,
            ) {
                <$name>::build_filter(state, outer);
            }

            fn build_access(_state: &Self::State, _out: &mut AccessParam) {}

            unsafe fn set_for_arche<'w>(
                state: &Self::State,
                cache: &mut Self::Cache<'w>,
                arche: &'w Archetype,
                table: &'w Table,
            ) {
                unsafe {
                    <$name>::set_for_arche(state, cache, arche, table);
                }
            }

            unsafe fn set_for_table<'w>(
                state: &Self::State,
                cache: &mut Self::Cache<'w>,
                table: &'w Table,
            ) {
                unsafe {
                    <$name>::set_for_table(state, cache, table);
                }
            }

            unsafe fn filter<'w>(
                state: &Self::State,
                cache: &mut Self::Cache<'w>,
                entity: Entity,
                table_row: TableRow,
            ) -> bool {
                unsafe {
                    <$name>::filter(state, cache, entity, table_row)
                }
            }
        }
    };
    ($num:literal : [$($index:tt : $name:ident),*]) => {
        #[cfg_attr(docsrs, doc(hidden))]
        impl<$($name: QueryFilter),*> InAnd for ($($name),*) {}

        #[cfg_attr(docsrs, doc(hidden))]
        unsafe impl<$($name: QueryFilter),*> QueryFilter for And<($($name),*)> {
            type State = ( $( <$name>::State ),* );
            type Cache<'world> = ( $( <$name>::Cache<'world> ),* );

            const COMPONENTS_ARE_DENSE: bool = {
                true $( && <$name>::COMPONENTS_ARE_DENSE )*
            };
            const ENABLE_ENTITY_FILTER: bool = {
                false $( || <$name>::ENABLE_ENTITY_FILTER )*
            };


            fn build_state(
                world: &mut World,
            ) -> Self::State {
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

            fn build_filter(
                state: &Self::State,
                outer: &mut Vec<FilterParamBuilder>,
            ) {

                let mut ret = Vec::<FilterParamBuilder>::new();
                ret.push(FilterParamBuilder::new());
                $({
                    let x = ::core::mem::take(&mut ret);
                    let mut y = Vec::<FilterParamBuilder>::new();
                    <$name>::build_filter(&state.$index, &mut y);
                    ret.reserve(x.len() * y.len());
                    x.iter().for_each(|a| {
                        y.iter().for_each(|b| {
                            if let Some(filter) = a.merge(b) {
                                ret.push(filter);
                            }
                        });
                    });
                })*

                outer.append(&mut ret);
            }

            fn build_access(_state: &Self::State, _out: &mut AccessParam) {}

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

            unsafe fn filter<'w>(
                state: &Self::State,
                cache: &mut Self::Cache<'w>,
                entity: Entity,
                table_row: TableRow,
            ) -> bool {
                unsafe {
                    true
                    $( && <$name>::filter(&state.$index, &mut cache.$index, entity, table_row) )*
                }
            }
        }
    };
}

voker_utils::range_invoke!(impl_tuple, 12);
