use core::any::TypeId;

use crate::borrow::{Mut, Ref};
use crate::component::{Component, StorageMode};
use crate::entity::Entity;
use crate::storage::{TableId, TableRow};
use crate::tick::Tick;
use crate::world::UnsafeWorld;

/// Generic component fetch contract used by entity and query accessors.
///
/// Implementations define how a fetch pattern is materialized for a single
/// entity/table row under the current world/tick context.
///
/// # Note
///
/// - If you fetch `&mut T`, the target is marked as changed immediately.
/// - If you fetch `Mut<T>`, the changed flag is deferred until you obtain an
///   inner mutable borrow. If you never borrow mutably, it is not marked changed.
///
/// # Examples
///
/// ```ignore
/// # use voker_ecs::borrow::{Mut, Ref};
/// # use voker_ecs::world::EntityMut;
/// # struct Foo;
/// # struct Bar;
/// # struct Baz;
/// let entity: EntityMut<'_> = todo!();
///
/// // Fetch one component.
/// let ret: Option<&Foo> = entity.fetch::<&Foo>();
///
/// // Fetch multiple components.
/// let ret: Option<(&Bar, &mut Baz)> = entity.fetch::<(&Bar, &mut Baz)>();
/// let ret: Option<(Ref<Bar>, Mut<Baz>)> = entity.fetch::<(Ref<Bar>, Mut<Baz>)>();
/// // Missing any required component returns `None`.
///
/// // If there are optional components, use `Option` wrapper:
/// let ret: Option<(&Foo, Option<&mut Bar>)> = entity.fetch::<(&Foo, Option<&mut Bar>)>();
/// // If `Bar` does not exist, only the optional part is `None`.
///
/// // Note that obtaining two mutable references to the same component is feasible,
/// // but this violates Rust aliasing requirements. Do not do this:
/// let ret: Option<(&mut Foo, &mut Foo)> = entity.fetch::<(&mut Foo, &mut Foo)>();
/// ```
///
/// # Safety
/// The implementer ensures the legality of access.
pub unsafe trait FetchComponents {
    /// The fetched output type.
    type Item<'a>;

    /// Fetches components for one entity according to the implementor pattern.
    ///
    /// `mutable` indicates whether mutable access is allowed by the caller
    /// (for example, `EntityRef` passes `false`, `EntityMut` passes `true`).
    ///
    /// # Safety
    /// The caller must ensure that `world`, `entity`, `table_id`, and
    /// `table_row` describe a valid and coherent location, and that borrow/
    /// aliasing rules for the requested mutability are upheld.
    unsafe fn fetch<'a>(
        mutable: bool,
        world: UnsafeWorld<'a>,
        entity: Entity,
        table_id: TableId,
        table_row: TableRow,
        last_run: Tick,
        this_run: Tick,
    ) -> Option<Self::Item<'a>>;
}

unsafe impl<T: Component> FetchComponents for &T {
    type Item<'a> = &'a T;

    unsafe fn fetch<'a>(
        _mutable: bool,
        world: UnsafeWorld<'a>,
        entity: Entity,
        table_id: TableId,
        table_row: TableRow,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Option<Self::Item<'a>> {
        let world = unsafe { world.read_only() };
        let id = world.components.get_id(TypeId::of::<T>())?;
        match T::STORAGE {
            StorageMode::Dense => {
                let tables = &world.storages.tables;
                let table = unsafe { tables.get_unchecked(table_id) };
                let table_col = table.get_table_col(id)?;
                let ptr = unsafe { table.get_data(table_row, table_col) };
                ptr.debug_assert_aligned::<T>();
                Some(unsafe { ptr.deref::<T>() })
            }
            StorageMode::Sparse => {
                let maps = &world.storages.maps;
                let map_id = maps.get_id(id)?;
                let map = unsafe { maps.get_unchecked(map_id) };
                let map_row = map.get_map_row(entity)?;
                let ptr = unsafe { map.get_data(map_row) };
                ptr.debug_assert_aligned::<T>();
                Some(unsafe { ptr.deref::<T>() })
            }
        }
    }
}

unsafe impl<T: Component> FetchComponents for &mut T {
    type Item<'a> = &'a mut T;

    unsafe fn fetch<'a>(
        mutable: bool,
        world: UnsafeWorld<'a>,
        entity: Entity,
        table_id: TableId,
        table_row: TableRow,
        last_run: Tick,
        this_run: Tick,
    ) -> Option<Self::Item<'a>> {
        if !T::MUTABLE || !mutable {
            return None;
        }

        let world = unsafe { world.data_mut() };
        let id = world.components.get_id(TypeId::of::<T>())?;
        match T::STORAGE {
            StorageMode::Dense => {
                let tables = &mut world.storages.tables;
                let table = unsafe { tables.get_unchecked_mut(table_id) };
                let table_col = table.get_table_col(id)?;
                let untyped = unsafe { table.get_mut(table_row, table_col, last_run, this_run) };
                Some(unsafe { untyped.with_type::<T>().into_inner() })
            }
            StorageMode::Sparse => {
                let maps = &mut world.storages.maps;
                let map_id = maps.get_id(id)?;
                let map = unsafe { maps.get_unchecked_mut(map_id) };
                let map_row = map.get_map_row(entity)?;
                let untyped = unsafe { map.get_mut(map_row, last_run, this_run) };
                Some(unsafe { untyped.with_type::<T>().into_inner() })
            }
        }
    }
}

unsafe impl<T: Component> FetchComponents for Ref<'_, T> {
    type Item<'a> = Ref<'a, T>;

    unsafe fn fetch<'a>(
        _mutable: bool,
        world: UnsafeWorld<'a>,
        entity: Entity,
        table_id: TableId,
        table_row: TableRow,
        last_run: Tick,
        this_run: Tick,
    ) -> Option<Self::Item<'a>> {
        let world = unsafe { world.read_only() };
        let id = world.components.get_id(TypeId::of::<T>())?;
        match T::STORAGE {
            StorageMode::Dense => {
                let tables = &world.storages.tables;
                let table = unsafe { tables.get_unchecked(table_id) };
                let table_col = table.get_table_col(id)?;
                let untyped = unsafe { table.get_ref(table_row, table_col, last_run, this_run) };
                Some(unsafe { untyped.with_type::<T>() })
            }
            StorageMode::Sparse => {
                let maps = &world.storages.maps;
                let map_id = maps.get_id(id)?;
                let map = unsafe { maps.get_unchecked(map_id) };
                let map_row = map.get_map_row(entity)?;
                let untyped = unsafe { map.get_ref(map_row, last_run, this_run) };
                Some(unsafe { untyped.with_type::<T>() })
            }
        }
    }
}

unsafe impl<T: Component> FetchComponents for Mut<'_, T> {
    type Item<'a> = Mut<'a, T>;

    unsafe fn fetch<'a>(
        mutable: bool,
        world: UnsafeWorld<'a>,
        entity: Entity,
        table_id: TableId,
        table_row: TableRow,
        last_run: Tick,
        this_run: Tick,
    ) -> Option<Self::Item<'a>> {
        if !T::MUTABLE || !mutable {
            return None;
        }

        let world = unsafe { world.data_mut() };
        let id = world.components.get_id(TypeId::of::<T>())?;
        match T::STORAGE {
            StorageMode::Dense => {
                let tables = &mut world.storages.tables;
                let table = unsafe { tables.get_unchecked_mut(table_id) };
                let table_col = table.get_table_col(id)?;
                let untyped = unsafe { table.get_mut(table_row, table_col, last_run, this_run) };
                Some(unsafe { untyped.with_type::<T>() })
            }
            StorageMode::Sparse => {
                let maps = &mut world.storages.maps;
                let map_id = maps.get_id(id)?;
                let map = unsafe { maps.get_unchecked_mut(map_id) };
                let map_row = map.get_map_row(entity)?;
                let untyped = unsafe { map.get_mut(map_row, last_run, this_run) };
                Some(unsafe { untyped.with_type::<T>() })
            }
        }
    }
}

unsafe impl<T: FetchComponents> FetchComponents for Option<T> {
    type Item<'a> = Option<T::Item<'a>>;

    unsafe fn fetch<'a>(
        mutable: bool,
        world: UnsafeWorld<'a>,
        entity: Entity,
        table_id: TableId,
        table_row: TableRow,
        last_run: Tick,
        this_run: Tick,
    ) -> Option<Self::Item<'a>> {
        unsafe {
            Some(T::fetch(
                mutable, world, entity, table_id, table_row, last_run, this_run,
            ))
        }
    }
}

macro_rules! impl_tuple {
    (0: []) => {};
    (1 : [ $index:tt : $name:ident ]) => {
        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 12 items long.")]
        unsafe impl<$name: FetchComponents> FetchComponents for ($name,) {
            type Item<'a> = (<$name>::Item<'a>,);

            unsafe fn fetch<'a>(
                mutable: bool,
                world: UnsafeWorld<'a>,
                entity: Entity,
                table_id: TableId,
                table_row: TableRow,
                last_run: Tick,
                this_run: Tick,
            ) -> Option<Self::Item<'a>> {
                unsafe {
                    Some((
                        <$name>::fetch(mutable, world, entity, table_id, table_row, last_run, this_run)?,
                    ))
                }
            }
        }
    };
    ($num:literal : [$($index:tt : $name:ident),*]) => {
        #[cfg_attr(docsrs, doc(hidden))]
        unsafe impl<$($name: FetchComponents),*> FetchComponents for ($($name,)*) {
            type Item<'a> = ( $( <$name>::Item<'a>, )* );

            unsafe fn fetch<'a>(
                mutable: bool,
                world: UnsafeWorld<'a>,
                entity: Entity,
                table_id: TableId,
                table_row: TableRow,
                last_run: Tick,
                this_run: Tick,
            ) -> Option<Self::Item<'a>> {
                unsafe {
                    Some((
                        $( <$name>::fetch(mutable, world, entity, table_id, table_row, last_run, this_run)?, )*
                    ))
                }
            }
        }
    };
}

voker_utils::range_invoke!(impl_tuple, 12);
