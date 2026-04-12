use alloc::vec::Vec;

use super::QueryFilter;
use crate::archetype::Archetype;
use crate::component::{Component, ComponentId, StorageMode};
use crate::entity::Entity;
use crate::storage::{Table, TableRow};
use crate::system::{AccessParam, FilterParamBuilder};
use crate::tick::Tick;
use crate::world::{UnsafeWorld, World};

// -----------------------------------------------------------------------------
// InWith

#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be used in `With<..>`",
    label = "Expected a Component or a tuple of 1-12 Components",
    note = "If there are more than 12 elements, use `And<..>` instead."
)]
pub trait InWith {}

// -----------------------------------------------------------------------------
// With

pub struct With<T: InWith>(T);

// -----------------------------------------------------------------------------
// With for Component

impl<T: Component> InWith for T {}

unsafe impl<T: Component> QueryFilter for With<T> {
    type State = ComponentId;
    type Cache<'world> = bool;

    const COMPONENTS_ARE_DENSE: bool = T::STORAGE.is_dense();
    const ENABLE_ENTITY_FILTER: bool = false;

    fn build_state(world: &mut World) -> Self::State {
        world.register_component::<T>()
    }

    fn fetch_state(world: &World) -> Option<Self::State> {
        world.get_component_id::<T>()
    }

    unsafe fn build_cache<'w>(
        _state: &Self::State,
        _world: UnsafeWorld<'w>,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Self::Cache<'w> {
        false
    }

    fn build_filter(state: &Self::State, outer: &mut Vec<FilterParamBuilder>) {
        let mut builder = FilterParamBuilder::new();
        builder.with(*state);
        outer.push(builder);
    }

    fn build_access(_state: &Self::State, _out: &mut AccessParam) {}

    unsafe fn set_for_arche<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        arche: &'w Archetype,
        _table: &'w Table,
    ) {
        match T::STORAGE {
            StorageMode::Dense => {
                *cache = arche.contains_dense_component(*state);
            }
            StorageMode::Sparse => {
                *cache = arche.contains_sparse_component(*state);
            }
        }
    }

    unsafe fn set_for_table<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        table: &'w Table,
    ) {
        debug_assert! {
            T::STORAGE.is_dense(),
            "Unexpected `set_for_table` for sparse component",
        }

        *cache = table.get_table_col(*state).is_some();
    }

    unsafe fn filter<'w>(
        _state: &Self::State,
        cache: &mut Self::Cache<'w>,
        _entity: Entity,
        _table_row: TableRow,
    ) -> bool {
        *cache
    }
}

// // -----------------------------------------------------------------------------
// // With for Tuple

macro_rules! to_component_id {
    ($_:ident) => {
        ComponentId
    };
}

macro_rules! impl_tuple {
    (0: []) => {};
    (1 : [ $index:tt : $name:ident ]) => {
        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 12 items long.")]
        impl<$name: Component> InWith for ($name,) {}

        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 12 items long.")]
        unsafe impl<$name: Component> QueryFilter for With<($name,)> {
            type State = ComponentId;
            type Cache<'world> = bool;

            const COMPONENTS_ARE_DENSE: bool = $name::STORAGE.is_dense();
            const ENABLE_ENTITY_FILTER: bool = false;

            fn build_state(world: &mut World) -> Self::State {
                world.register_component::<$name>()
            }

            fn fetch_state(world: &World) -> Option<Self::State> {
                world.get_component_id::<$name>()
            }

            unsafe fn build_cache<'w>(
                _state: &Self::State,
                _world: UnsafeWorld<'w>,
                _last_run: Tick,
                _this_run: Tick,
            ) -> Self::Cache<'w> {
                false
            }

            fn build_filter(
                state: &Self::State,
                outer: &mut Vec<FilterParamBuilder>,
            ) {
                let mut builder = FilterParamBuilder::new();
                builder.with(*state);
                outer.push(builder);
            }

            fn build_access(_state: &Self::State, _out: &mut AccessParam) {}

            unsafe fn set_for_arche<'w>(
                state: &Self::State,
                cache: &mut Self::Cache<'w>,
                arche: &'w Archetype,
                _table: &'w Table,
            ) {
                match <$name>::STORAGE {
                    StorageMode::Dense => {
                        *cache = arche.contains_dense_component(*state);
                    },
                    StorageMode::Sparse => {
                        *cache = arche.contains_sparse_component(*state);
                    },
                }
            }

            unsafe fn set_for_table<'w>(
                state: &Self::State,
                cache: &mut Self::Cache<'w>,
                table: &'w Table,
            ) {
                debug_assert!{
                    <$name>::STORAGE.is_dense(),
                    "Unexpected `set_for_table` for sparse component",
                }

                *cache = table.get_table_col(*state).is_some();
            }

            unsafe fn filter<'w>(
                _state: &Self::State,
                cache: &mut Self::Cache<'w>,
                _entity: Entity,
                _table_row: TableRow,
            ) -> bool {
                *cache
            }
        }
    };
    ($num:literal : [$($index:tt : $name:ident),*]) => {
        #[cfg_attr(docsrs, doc(hidden))]
        impl<$($name: Component),*> InWith for ($($name),*) {}

        #[cfg_attr(docsrs, doc(hidden))]
        unsafe impl<$($name: Component),*> QueryFilter for With<($($name),*)> {
            type State = ( $( to_component_id!{ $name } ),* );
            type Cache<'world> = bool;

            const COMPONENTS_ARE_DENSE: bool = {
                true $( && <$name>::STORAGE.is_dense() )*
            };
            const ENABLE_ENTITY_FILTER: bool = false;

            fn build_state(world: &mut World) -> Self::State {
                ( $( world.register_component::<$name>(), )* )
            }

            fn fetch_state(world: &World) -> Option<Self::State> {
                Some(( $( world.get_component_id::<$name>()?, )* ))
            }

            unsafe fn build_cache<'w>(
                _state: &Self::State,
                _world: UnsafeWorld<'w>,
                _last_run: Tick,
                _this_run: Tick,
            ) -> Self::Cache<'w> {
                false
            }

            fn build_filter(
                state: &Self::State,
                outer: &mut Vec<FilterParamBuilder>,
            ) {
                let mut builder = FilterParamBuilder::new();
                $( builder.with(state.$index); )*
                outer.push(builder);
            }

            fn build_access(_state: &Self::State, _out: &mut AccessParam) {}

            unsafe fn set_for_arche<'w>(
                state: &Self::State,
                cache: &mut Self::Cache<'w>,
                arche: &'w Archetype,
                _table: &'w Table,
            ) {
                *cache = true;
                $(
                    match <$name>::STORAGE {
                        StorageMode::Dense => {
                            *cache &= arche.contains_dense_component(state.$index);
                        },
                        StorageMode::Sparse => {
                            *cache &= arche.contains_sparse_component(state.$index);
                        },
                    }
                )*
            }

            unsafe fn set_for_table<'w>(
                state: &Self::State,
                cache: &mut Self::Cache<'w>,
                table: &'w Table,
            ) {
                *cache = true;
                $(
                    debug_assert!{
                        <$name>::STORAGE.is_dense(),
                        "Unexpected `set_for_table` for sparse component",
                    }

                    *cache &= table.get_table_col(state.$index).is_some();
                )*
            }

            unsafe fn filter<'w>(
                _state: &Self::State,
                cache: &mut Self::Cache<'w>,
                _entity: Entity,
                _table_row: TableRow,
            ) -> bool {
                *cache
            }
        }
    };
}

voker_utils::range_invoke!(impl_tuple, 12);
