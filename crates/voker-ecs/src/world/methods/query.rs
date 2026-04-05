use core::any::TypeId;

use crate::query::{Query, QueryData, QueryFilter, QueryState};
use crate::system::SystemParam;
use crate::world::{UnsafeWorld, World};

impl World {
    /// Creates a fresh [`QueryState`] from query parameters.
    ///
    /// This function does **not** cache the query state as a world resource.
    /// Use this when you want one-off query setup without persistent caching.
    pub fn query_once<D: QueryData, F: QueryFilter>(&mut self) -> QueryState<D, F> {
        <QueryState<D, F>>::new(self)
    }

    /// Returns a cached [`QueryState`] resource, creating it if missing.
    ///
    /// [`World::query`] and [`World::query_with`] call this automatically to
    /// avoid repeated initialization and archetype-filter setup costs.
    ///
    /// If you do not want caching, use [`World::query_once`] for ad-hoc
    /// query construction.
    ///
    /// Note: when `Query` is used as a system parameter, its query state is
    /// stored on the system instance, not in [`World`].
    pub fn cache_query_state<D: QueryData + 'static, F: QueryFilter + 'static>(
        &mut self,
    ) -> &mut QueryState<D, F> {
        let unsafe_world = self.unsafe_world();
        let data_mut = unsafe { unsafe_world.data_mut() };
        if let Some(state) = data_mut.resource_mut::<QueryState<D, F>>() {
            state.into_inner()
        } else {
            let full_mut = unsafe { unsafe_world.full_mut() };
            let state = <QueryState<D, F>>::new(full_mut);
            full_mut.insert_resource(state)
        }
    }

    /// Clears a cached query state created by [`World::cache_query_state`].
    ///
    /// If no such cached state exists, this is a no-op.
    pub fn clear_query_state<D: QueryData + 'static, F: QueryFilter + 'static>(&mut self) {
        let type_id = TypeId::of::<QueryState<D, F>>();
        if let Some(id) = self.resources.get_id(type_id)
            && let Some(data) = self.storages.res_set.get_mut(id)
        {
            unsafe {
                data.clear();
            }
        }
    }

    /// Creates a cached query with no filter.
    ///
    /// This is shorthand for `query_with::<D, ()>()`. Internally, it updates a
    /// cached [`QueryState`] before constructing the runtime query parameter.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::component::Component;
    /// # use voker_ecs::world::World;
    /// # #[derive(Component, Debug)]
    /// # struct Foo;
    /// #
    /// # let mut world = World::alloc();
    /// world.spawn(Foo);
    /// world.spawn(Foo);
    ///
    /// let query = world.query::<&Foo>();
    /// assert_eq!(query.iter().count(), 2);
    /// ```
    pub fn query<D: QueryData + 'static>(&mut self) -> Query<'_, '_, D> {
        let world: UnsafeWorld<'_> = self.unsafe_world();
        let state = unsafe { world.full_mut().cache_query_state::<D, ()>() };
        let read_only_world = unsafe { world.read_only() };
        state.update(read_only_world);
        let last_run = read_only_world.last_run();
        let this_run = read_only_world.this_run();

        unsafe { <Query<D> as SystemParam>::build_param(world, state, last_run, this_run).unwrap() }
    }

    /// Creates a cached query with an explicit filter.
    ///
    /// Use this when you need conditional matching (`With`, `Without`, `And`,
    /// `Or`, etc.) in addition to the query data.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::component::Component;
    /// # use voker_ecs::query::With;
    /// # use voker_ecs::world::World;
    /// # #[derive(Component, Debug)]
    /// # struct Foo;
    /// # #[derive(Component, Debug)]
    /// # struct Bar(u64);
    /// #
    /// # let mut world = World::alloc();
    /// world.spawn((Foo, Bar(1)));
    /// world.spawn(Bar(2));
    ///
    /// let query = world.query_with::<&Bar, With<Foo>>();
    /// assert_eq!(query.iter().count(), 1);
    /// for bar in query {
    ///     assert_eq!(bar.0, 1);
    /// }
    /// ```
    pub fn query_with<D: QueryData + 'static, F: QueryFilter + 'static>(
        &mut self,
    ) -> Query<'_, '_, D, F> {
        let world: UnsafeWorld<'_> = self.unsafe_world();
        let state = unsafe { world.full_mut().cache_query_state::<D, F>() };
        let read_only_world = unsafe { world.read_only() };
        state.update(read_only_world);
        let last_run = read_only_world.last_run();
        let this_run = read_only_world.this_run();

        unsafe {
            <Query<D, F> as SystemParam>::build_param(world, state, last_run, this_run).unwrap()
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::borrow::{Mut, Ref};
    use crate::component::{Component, ComponentStorage};
    use crate::entity::Entity;
    use crate::query::{And, Or, With, Without};
    use crate::tick::DetectChanges;
    use crate::world::{EntityMut, EntityRef, World};
    use alloc::string::String;
    use alloc::vec::Vec;

    #[derive(Debug, PartialEq, Eq)]
    struct Foo;

    #[derive(Debug, PartialEq, Eq)]
    struct Bar(u64);

    #[derive(Debug, PartialEq, Eq)]
    struct Baz(String);

    #[derive(Debug, PartialEq)]
    struct Qux(f32);

    #[derive(Debug, PartialEq, Eq)]
    struct Zaz(i32);

    impl Component for Foo {}
    impl Component for Bar {}
    impl Component for Baz {
        const STORAGE: ComponentStorage = ComponentStorage::Sparse;
    }
    impl Component for Qux {}
    impl Component for Zaz {}

    #[test]
    fn query_raw_ref() {
        let mut world = World::alloc();

        world.spawn((Foo, Bar(100)));
        world.spawn((Foo, Bar(200)));
        world.spawn((Baz(String::from("no foo")),));
        world.update_tick();

        let query = world.query::<&Foo>();

        assert_eq!(query.into_iter().count(), 2);

        let query = world.query::<&Bar>();
        assert_eq!(query.into_iter().count(), 2);
    }

    #[test]
    fn query_raw_mut() {
        let mut world = World::alloc();

        world.spawn((Foo, Bar(100)));
        world.spawn((Foo, Bar(200)));
        world.update_tick();

        let query = world.query::<&mut Bar>();
        for bar in query {
            bar.0 += 50;
        }

        let query = world.query::<&Bar>();
        let values: Vec<u64> = query.into_iter().map(|bar| bar.0).collect();
        assert!(values.contains(&150));
        assert!(values.contains(&250));
    }

    #[test]
    fn query_ref() {
        let mut world = World::alloc();

        world.spawn((Foo, Bar(100)));
        world.spawn((Foo, Bar(200)));
        world.update_tick();

        let query = world.query::<Ref<Bar>>();
        for bar_ref in query {
            assert!(!bar_ref.is_changed());
        }
    }

    #[test]
    fn query_mut() {
        let mut world = World::alloc();

        world.spawn((Foo, Bar(100)));
        world.update_tick();

        let query = world.query::<Mut<Bar>>();
        for mut bar_mut in query.into_iter() {
            assert!(!bar_mut.is_changed());
            bar_mut.as_mut().0 = 999;
            assert!(bar_mut.is_changed());
        }

        let query = world.query::<&Bar>();
        assert_eq!(query.into_iter().next().unwrap().0, 999);
    }

    #[test]
    fn query_entity() {
        let mut world = World::alloc();

        let e1 = world.spawn((Foo, Bar(100))).entity();
        let e2 = world.spawn((Foo, Bar(200))).entity();
        world.update_tick();

        let query = world.query::<Entity>();
        let entities: Vec<_> = query.into_iter().collect();
        assert_eq!(entities.len(), 2);
        assert!(entities.contains(&e1));
        assert!(entities.contains(&e2));
    }

    #[test]
    fn query_entity_ref() {
        let mut world = World::alloc();

        world.spawn((Foo, Bar(100), Baz(String::from("a"))));
        world.spawn((Foo, Bar(200)));
        world.update_tick();

        let query = world.query::<EntityRef>();
        for entity_ref in query {
            assert!(entity_ref.contains::<Foo>());
            if entity_ref.contains::<Baz>() {
                let baz = entity_ref.get::<Baz>().unwrap();
                assert_eq!(baz.0, "a");
            }
        }
    }

    #[test]
    fn query_entity_mut() {
        let mut world = World::alloc();

        world.spawn((Foo, Bar(100)));
        world.spawn((Foo, Bar(200)));
        world.update_tick();

        let query = world.query::<EntityMut>();
        for mut entity_mut in query {
            if let Some(mut bar) = entity_mut.get_mut::<Bar>() {
                bar.0 += 50;
            }

            assert!(!entity_mut.contains::<Zaz>());
        }

        let query = world.query::<&Bar>();
        let bars: Vec<u64> = query.into_iter().map(|b| b.0).collect();
        assert!(bars.contains(&150));
        assert!(bars.contains(&250));
    }

    #[test]
    fn filter_with_single() {
        let mut world = World::alloc();

        world.spawn((Foo, Bar(100), Baz(String::from("a"))));
        world.spawn((Foo, Bar(200)));
        world.spawn((Bar(300), Baz(String::from("b"))));
        world.update_tick();

        let query = world.query_with::<&Bar, With<Foo>>();
        assert_eq!(query.into_iter().count(), 2);

        let query = world.query_with::<&Foo, With<Baz>>();
        assert_eq!(query.into_iter().count(), 1);
    }

    #[test]
    fn filter_with_tuple() {
        let mut world = World::alloc();

        world.spawn((Foo, Bar(100), Baz(String::from("a")), Qux(1.0)));
        world.spawn((Foo, Bar(200), Baz(String::from("b"))));
        world.spawn((Foo, Bar(300), Qux(3.0)));
        world.spawn((Foo, Baz(String::from("c")), Qux(4.0)));
        world.update_tick();

        let query = world.query_with::<&Foo, With<(Bar, Baz)>>();
        assert_eq!(query.into_iter().count(), 2);

        let query = world.query_with::<&Foo, With<(Bar, Qux)>>();
        assert_eq!(query.into_iter().count(), 2);
    }

    #[test]
    fn filter_without_single() {
        let mut world = World::alloc();

        world.spawn((Foo, Bar(100), Baz(String::from("a"))));
        world.spawn((Foo, Bar(200)));
        world.spawn((Bar(300), Baz(String::from("b"))));
        world.update_tick();

        let query = world.query_with::<&Bar, Without<Foo>>();
        assert_eq!(query.into_iter().count(), 1);

        let query = world.query_with::<&Foo, Without<Baz>>();
        assert_eq!(query.into_iter().count(), 1);
    }

    #[test]
    fn filter_without_tuple() {
        let mut world = World::alloc();

        world.spawn((Foo, Bar(100), Baz(String::from("a")), Qux(1.0)));
        world.spawn((Foo, Bar(200), Baz(String::from("b"))));
        world.spawn((Foo, Bar(300), Qux(3.0)));
        world.spawn((Foo, Baz(String::from("c")), Qux(4.0)));
        world.update_tick();

        let query = world.query_with::<&Foo, Without<(Baz, Qux)>>();
        assert_eq!(query.into_iter().count(), 0);

        let query = world.query_with::<&Foo, Without<(Bar,)>>();
        assert_eq!(query.into_iter().count(), 1);
    }

    #[test]
    fn filter_or() {
        let mut world = World::alloc();

        world.spawn((Foo, Bar(100), Baz(String::from("a"))));
        world.spawn((Foo, Bar(200)));
        world.spawn((Foo, Qux(3.0)));
        world.spawn((Foo, Baz(String::from("c")), Qux(4.0)));
        world.update_tick();

        let query = world.query_with::<&Foo, Or<(With<Bar>, With<Qux>)>>();
        assert_eq!(query.into_iter().count(), 4);

        let query = world.query_with::<&Foo, Or<(With<Bar>, With<Baz>)>>();
        assert_eq!(query.into_iter().count(), 3);
    }

    #[test]
    fn filter_and() {
        let mut world = World::alloc();

        world.spawn((Foo, Bar(100), Baz(String::from("a")), Qux(1.0)));
        world.spawn((Foo, Bar(200), Baz(String::from("b"))));
        world.spawn((Foo, Bar(300), Qux(3.0)));
        world.spawn((Foo, Baz(String::from("c")), Qux(4.0)));
        world.update_tick();

        let query = world.query_with::<&Foo, And<(With<Bar>, With<Baz>)>>();
        assert_eq!(query.into_iter().count(), 2);

        let query = world.query_with::<&Foo, And<(With<Bar>, With<Qux>)>>();
        assert_eq!(query.into_iter().count(), 2);

        let query = world.query_with::<&Foo, And<(With<Bar>, With<Baz>, With<Qux>)>>();
        assert_eq!(query.into_iter().count(), 1);
    }

    #[test]
    fn filter_nested_conditions() {
        let mut world = World::alloc();

        world.spawn((Foo, Bar(100), Baz(String::from("a")), Qux(1.0)));
        world.spawn((Foo, Bar(200), Baz(String::from("b"))));
        world.spawn((Foo, Bar(300), Qux(3.0)));
        world.spawn((Foo, Baz(String::from("c")), Qux(4.0)));
        world.spawn((Foo, Zaz(42)));
        world.update_tick();

        let query = world
            .query_with::<&Foo, Or<(And<(With<Bar>, Or<(With<Baz>, With<Qux>)>)>, With<Zaz>)>>();

        assert_eq!(query.into_iter().count(), 4);

        let query = world.query_with::<&Zaz, ()>();
        assert_eq!(query.into_iter().count(), 1);
    }

    #[test]
    fn filter_mixed_with_and_without() {
        let mut world = World::alloc();

        world.spawn((Foo, Bar(100), Baz(String::from("a")), Qux(1.0)));
        world.spawn((Foo, Bar(200), Baz(String::from("b"))));
        world.spawn((Foo, Bar(300), Qux(3.0)));
        world.spawn((Foo, Baz(String::from("c")), Qux(4.0)));
        world.spawn((Foo, Zaz(42)));
        world.update_tick();

        let query =
            world.query_with::<&Foo, And<(With<Bar>, Without<Baz>, Or<(With<Qux>, With<Zaz>)>)>>();

        assert_eq!(query.into_iter().count(), 1);

        let query = world.query_with::<&Qux, ()>();
        let qux_values: Vec<f32> = query.into_iter().map(|q| q.0).collect();
        assert!(qux_values.contains(&3.0));
    }
}
