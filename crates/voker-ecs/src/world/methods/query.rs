use crate::query::{Query, QueryData, QueryFilter, QueryState};
use crate::world::{UnsafeWorld, World};

impl World {
    /// Returns a cached [`QueryState`] resource, creating it if missing.
    ///
    /// [`World::query`] and [`World::query_with`] call this automatically to
    /// avoid repeated initialization and archetype/filter setup costs.
    ///
    /// For a one-off query state value, call [`World::query_state`] and use
    /// the returned state directly.
    ///
    /// Note: when `Query` is used as a system parameter, its query state is
    /// stored on the system instance, not in [`World`].
    pub fn register_query<D, F>(&mut self) -> &mut QueryState<D, F>
    where
        D: QueryData + 'static,
        F: QueryFilter + 'static,
    {
        let unsafe_world = self.unsafe_world();
        let data_mut = unsafe { unsafe_world.data_mut() };
        if let Some(mut state) = data_mut.get_resource_mut::<QueryState<D, F>>() {
            let read_world = unsafe { unsafe_world.read_only() };
            state.update(read_world);
            state.into_inner()
        } else {
            let full_mut = unsafe { unsafe_world.full_mut() };
            let state = <QueryState<D, F>>::build(full_mut);
            full_mut.insert_resource(state)
        }
    }

    /// Removes a cached query state created by [`World::register_query`].
    ///
    /// If no matching cached state exists, this is a no-op.
    pub fn unregister_query<D, F>(&mut self)
    where
        D: QueryData + 'static,
        F: QueryFilter + 'static,
    {
        self.drop_resource::<QueryState<D, F>>();
    }

    /// Create a `QueryState` if all state can be initialized.
    ///
    /// It's always success because we hold exclusive reference of world.
    ///
    /// If the state is unregistered, `register_query` will be called.
    pub fn query_state<D, F>(&mut self) -> QueryState<D, F>
    where
        D: QueryData + 'static,
        F: QueryFilter + 'static,
    {
        self.register_query::<D, F>().clone()
    }

    /// Create a `QueryState` if all state can be initialized.
    ///
    /// This function can be used for `&World`, be may return `None`
    /// if some component (need to access) is unregistered.
    ///
    /// For example, the query usually used for access component,
    /// then the state required the Component ID is registered.
    ///
    /// We hold a immutable world, cannot register component id,
    /// so this function return None is required Component ID
    /// can not be find through [`World::get_component_id`].
    ///
    /// In the future, we may add auto-register implementation
    /// for components, so this function usually return Some.
    pub fn get_query_state<D, F>(&self) -> Option<QueryState<D, F>>
    where
        D: QueryData + 'static,
        F: QueryFilter + 'static,
    {
        if let Some(state) = self.get_resource::<QueryState<D, F>>() {
            let mut state = state.clone();
            state.update(self);
            Some(state)
        } else {
            QueryState::<D, F>::try_build(self)
        }
    }

    /// Creates a registered query view with no explicit filter.
    ///
    /// The query state is registered on first use if it does not exist.
    ///
    /// This is shorthand for `query_with::<D, ()>()`. Internally, it updates a
    /// cached [`QueryState`] before constructing the runtime query parameter.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::prelude::*;
    /// # #[derive(Component, Clone, Debug)]
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
        let state = unsafe { world.full_mut().register_query::<D, ()>() };
        let read_only_world = unsafe { world.read_only() };
        state.update(read_only_world);
        let last_run = read_only_world.last_run();
        let this_run = read_only_world.this_run();

        unsafe { Query::<D, ()>::new(world, state, last_run, this_run) }
    }

    /// Creates a registered query view with an explicit filter.
    ///
    /// The query state is registered on first use if it does not exist.
    ///
    /// Use this when you need conditional matching (`With`, `Without`, `And`,
    /// `Or`, etc.) in addition to the query data.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::prelude::*;
    /// # #[derive(Component, Clone, Debug)]
    /// # struct Foo;
    /// # #[derive(Component, Clone, Debug)]
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
    pub fn query_with<D, F>(&mut self) -> Query<'_, '_, D, F>
    where
        D: QueryData + 'static,
        F: QueryFilter + 'static,
    {
        let world: UnsafeWorld<'_> = self.unsafe_world();
        let state = unsafe { world.full_mut().register_query::<D, F>() };
        let read_only_world = unsafe { world.read_only() };
        state.update(read_only_world);
        let last_run = read_only_world.last_run();
        let this_run = read_only_world.this_run();

        unsafe { Query::<D, F>::new(world, state, last_run, this_run) }
    }

    /// Creates a registered query view with no explicit filter.
    ///
    /// Return `None` if the query state is unregistered.
    ///
    /// This function can be used for `DeferredWorld`.
    pub fn try_query<D: QueryData + 'static>(&mut self) -> Option<Query<'_, '_, D>> {
        let unsafe_world = self.unsafe_world();
        let data_mut = unsafe { unsafe_world.data_mut() };
        let state = data_mut.get_resource_mut::<QueryState<D>>()?;
        let state = state.into_inner();
        let world = unsafe { unsafe_world.data_mut() };
        state.update(world);
        Some(state.query_mut(world))
    }

    /// Creates a registered query view with with an explicit filter.
    ///
    /// Return `None` if the query state is unregistered.
    ///
    /// This function can be used for `DeferredWorld`.
    pub fn try_query_with<D, F>(&mut self) -> Option<Query<'_, '_, D, F>>
    where
        D: QueryData + 'static,
        F: QueryFilter + 'static,
    {
        let unsafe_world = self.unsafe_world();
        let data_mut = unsafe { unsafe_world.data_mut() };
        let state = data_mut.get_resource_mut::<QueryState<D, F>>()?;
        let state = state.into_inner();
        let world = unsafe { unsafe_world.data_mut() };
        state.update(world);
        Some(state.query_mut(world))
    }
}

#[cfg(test)]
mod tests {
    use crate::borrow::{Mut, Ref};
    use crate::component::Component;
    use crate::entity::Entity;
    use crate::query::{And, Or, With, Without};
    use crate::tick::DetectChanges;
    use crate::world::{EntityMut, EntityRef, World};
    use alloc::string::String;
    use alloc::vec::Vec;

    #[derive(Component, Clone, Debug, PartialEq, Eq)]
    struct Foo;

    #[derive(Component, Clone, Debug, PartialEq, Eq)]
    struct Bar(u64);

    #[derive(Component, Clone, Debug, PartialEq, Eq)]
    #[component(storage = "sparse")]
    struct Baz(String);

    #[derive(Component, Clone, Debug, PartialEq)]
    struct Qux(f32);

    #[derive(Component, Clone, Debug, PartialEq, Eq)]
    struct Zaz(i32);

    #[test]
    fn query_raw_ref() {
        let mut world = World::alloc();

        world.spawn((Foo, Bar(100)));
        world.spawn((Foo, Bar(200)));
        world.spawn((Baz(String::from("no foo")),));
        world.reset_last_run();

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
        world.reset_last_run();

        let query = world.query::<&mut Bar>();
        for mut bar in query {
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
        world.reset_last_run();

        let query = world.query::<Ref<Bar>>();
        for bar_ref in query {
            assert!(!bar_ref.is_changed());
        }
    }

    #[test]
    fn query_mut() {
        let mut world = World::alloc();

        world.spawn((Foo, Bar(100)));
        world.reset_last_run();

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
        world.reset_last_run();

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
        world.reset_last_run();

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
        world.reset_last_run();

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
        world.reset_last_run();

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
        world.reset_last_run();

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
        world.reset_last_run();

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
        world.reset_last_run();

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
        world.reset_last_run();

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
        world.reset_last_run();

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
        world.reset_last_run();

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
        world.reset_last_run();

        let query =
            world.query_with::<&Foo, And<(With<Bar>, Without<Baz>, Or<(With<Qux>, With<Zaz>)>)>>();

        assert_eq!(query.into_iter().count(), 1);

        let query = world.query_with::<&Qux, ()>();
        let qux_values: Vec<f32> = query.into_iter().map(|q| q.0).collect();
        assert!(qux_values.contains(&3.0));
    }

    // #[test]
    // fn query_get_and_get_mut() {
    //     let mut world = World::alloc();

    //     let e1 = world.spawn((Foo, Bar(10))).entity();
    //     let e2 = world.spawn((Foo,)).entity();

    //     {
    //         let query = world.query_with::<&Bar, With<Foo>>();
    //         assert_eq!(query.get(e1).unwrap().0, 10);
    //         assert!(matches!(
    //             query.get(e2),
    //             Err(QueryEntityError::QueryMismatch(entity)) if entity == e2
    //         ));
    //     }

    //     {
    //         let mut query = world.query::<&mut Bar>();
    //         query.get_mut(e1).unwrap().0 = 99;
    //     }

    //     let query = world.query::<&Bar>();
    //     assert_eq!(query.get(e1).unwrap().0, 99);
    // }

    // #[test]
    // fn query_get_many_mut_rejects_duplicates() {
    //     let mut world = World::alloc();
    //     let e = world.spawn((Foo, Bar(1))).entity();

    //     let mut query = world.query::<&mut Bar>();
    //     let err = query.get_many([e, e]).unwrap_err();
    //     assert!(matches!(err, QueryEntityError::DuplicateEntity(entity) if entity == e));
    // }

    // #[test]
    // fn query_get_single_variants() {
    //     let mut world = World::alloc();

    //     {
    //         let query = world.query::<&Foo>();
    //         assert!(matches!(
    //             query.get_single(),
    //             Err(QuerySingleError::NoEntities)
    //         ));
    //     }

    //     let e = world.spawn((Foo,)).entity();
    //     {
    //         let query = world.query::<Entity>();
    //         assert_eq!(query.get_single().unwrap(), e);
    //     }

    //     world.spawn((Foo,));
    //     {
    //         let query = world.query::<&Foo>();
    //         assert!(matches!(
    //             query.get_single(),
    //             Err(QuerySingleError::MultipleEntities)
    //         ));
    //     }
    // }

    // #[test]
    // fn single_system_param_reports_system_param_error() {
    //     let mut world = World::alloc();

    //     fn need_exact_one(_single: Single<&Foo>) {}

    //     let err = world.run_system(need_exact_one).unwrap_err();
    //     assert!(matches!(err, SystemError::Param(_)));

    //     world.spawn((Foo,));
    //     world.run_system(need_exact_one).unwrap();

    //     world.spawn((Foo,));
    //     let err = world.run_system(need_exact_one).unwrap_err();
    //     assert!(matches!(err, SystemError::Param(_)));
    // }
}
