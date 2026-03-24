use alloc::vec::Vec;

use super::QueryFilter;
use crate::archetype::Archetype;
use crate::component::{Component, ComponentId, ComponentStorage};
use crate::entity::Entity;
use crate::storage::{Table, TableRow};
use crate::system::{AccessParam, FilterParamBuilder};
use crate::tick::Tick;
use crate::world::{UnsafeWorld, World};

// -----------------------------------------------------------------------------
// InWithout

#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be used in `Without<..>`",
    label = "Expected a Component or a tuple of 1-12 Components",
    note = "If there are more than 12 elements, use `And<..>` instead."
)]
pub trait InWithout {}

// -----------------------------------------------------------------------------
// With

pub struct Without<T: InWithout>(T);

// -----------------------------------------------------------------------------
// With for Component

#[cold]
#[inline(never)]
fn set_table_for_sparse() -> ! {
    unreachable!("Unexpected `set_for_table` for sparse component");
}

impl<T: Component> InWithout for T {}

unsafe impl<T: Component> QueryFilter for Without<T> {
    type State = ComponentId;
    type Cache<'world> = bool;

    const COMPONENTS_ARE_DENSE: bool = T::STORAGE.is_dense();
    const ENABLE_ENTITY_FILTER: bool = false;

    fn build_state(world: &mut World) -> Self::State {
        world.register_component::<T>()
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
        builder.without(*state);
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
            ComponentStorage::Dense => {
                *cache = !arche.contains_dense_component(*state);
            }
            ComponentStorage::Sparse => {
                *cache = !arche.contains_sparse_component(*state);
            }
        }
    }

    unsafe fn set_for_table<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        table: &'w Table,
    ) {
        match T::STORAGE {
            ComponentStorage::Dense => {
                *cache = table.get_table_col(*state).is_none();
            }
            ComponentStorage::Sparse => {
                // *cache = false;
                set_table_for_sparse();
            }
        }
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

// -----------------------------------------------------------------------------
// With for Tuple

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
        impl<$name: Component> InWithout for ($name,) {}

        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 12 items long.")]
        unsafe impl<$name: Component> QueryFilter for Without<($name,)> {
            type State = ComponentId;
            type Cache<'world> = bool;
            const COMPONENTS_ARE_DENSE: bool = <$name>::STORAGE.is_dense();
            const ENABLE_ENTITY_FILTER: bool = false;

            fn build_state(
                world: &mut World,
            ) -> Self::State {
                world.register_component::<$name>()
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
                builder.without(*state);
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
                    ComponentStorage::Dense => {
                        *cache = !arche.contains_dense_component(*state);
                    },
                    ComponentStorage::Sparse => {
                        *cache = !arche.contains_sparse_component(*state);
                    },
                }
            }

            unsafe fn set_for_table<'w>(
                state: &Self::State,
                cache: &mut Self::Cache<'w>,
                table: &'w Table,
            ) {
                match <$name>::STORAGE {
                    ComponentStorage::Dense => {
                        *cache = table.get_table_col(*state).is_none();
                    },
                    ComponentStorage::Sparse => {
                        // *cache = false;
                        set_table_for_sparse();
                    },
                }
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
        impl<$($name: Component),*> InWithout for ($($name),*) {}

        #[cfg_attr(docsrs, doc(hidden))]
        unsafe impl<$($name: Component),*> QueryFilter for Without<($($name),*)> {
            type State = ( $( to_component_id!{ $name } ),* );
            type Cache<'world> = bool;
            const COMPONENTS_ARE_DENSE: bool = {
                true $( && <$name>::STORAGE.is_dense() )*
            };
            const ENABLE_ENTITY_FILTER: bool = false;

            fn build_state(
                world: &mut World,
            ) -> Self::State {
                ( $( world.register_component::<$name>(), )* )
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
                $( builder.without(state.$index); )*
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
                        ComponentStorage::Dense => {
                            *cache &= !arche.contains_dense_component(state.$index);
                        },
                        ComponentStorage::Sparse => {
                            *cache &= !arche.contains_sparse_component(state.$index);
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
                    match <$name>::STORAGE {
                        ComponentStorage::Dense => {
                            *cache &= table.get_table_col(state.$index).is_none();
                        },
                        ComponentStorage::Sparse => {
                            // *cache = false;
                            set_table_for_sparse();
                        },
                    }
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
