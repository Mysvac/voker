//! Query types, state, iteration, and filters.
//!
//! Query execution is split into two stages:
//! - storage-level filtering (archetype/table selection), then
//! - optional entity-level filtering.
//!
//! [`QueryData`] defines what is fetched (`&T`, `&mut T`, wrappers, tuples),
//! while [`QueryFilter`] defines which entities are included (`With`, `Without`,
//! `Added`, `Changed`, logical `And`/`Or`).
//!
//! Note: mutable component query forms (`&mut T`, `Option<&mut T>`) fetch
//! [`crate::borrow::Mut`] wrappers at runtime to preserve change-tracking
//! metadata.
//!
//! [`QueryState`] caches compiled filter/data state and supports incremental
//! updates as archetypes are added.
//!
//! In addition to iterator-based access, [`Query`] and [`QueryState`] provide
//! entity-targeted APIs (`get`, `get_many`, `get_single`, etc.).
//!
//! [`Single`] offers a system-parameter form that guarantees exactly one match
//! and reports failure as [`crate::system::SystemParamError`].

// -----------------------------------------------------------------------------
// Modules

mod data;
mod error;
mod filter;
mod iter;
mod query;
mod single;
mod state;

// -----------------------------------------------------------------------------
// Exports

pub use data::{QueryData, ReadOnlyQueryData};
pub use error::{QueryEntityError, QuerySingleError};
pub use filter::{Added, And, Changed, Or, QueryFilter, With, Without};
pub use iter::QueryIter;
pub use query::Query;
pub use single::Single;
pub use state::QueryState;
pub use voker_ecs_derive::QueryData;

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use crate::borrow::Mut;
    use crate::derive::{Component, QueryData};
    use crate::entity::Entity;
    use crate::world::World;

    #[derive(Component, Clone, Debug, PartialEq)]
    struct Position {
        x: f32,
        y: f32,
    }

    #[derive(Component, Clone, Debug, PartialEq)]
    struct Velocity {
        x: f32,
        y: f32,
    }

    #[derive(QueryData)]
    #[query_data(readonly)]
    #[expect(dead_code, reason = "test only")]
    struct EntityOnly {
        entity: Entity,
    }

    #[derive(QueryData)]
    #[query_data(readonly)]
    struct ReadVelocity<'w> {
        velocity: &'w Velocity,
    }

    #[derive(QueryData)]
    struct MoveData<'w> {
        position: Mut<'w, Position>,
        velocity: &'w Velocity,
    }

    #[test]
    fn derive_query_data_supports_no_lifetime_type() {
        let mut world = World::alloc();
        world.spawn(());
        world.spawn(());

        let query = world.query::<EntityOnly>();
        assert_eq!(query.iter().count(), 2);
    }

    #[test]
    fn derive_query_data_supports_single_w_lifetime_readonly_query_iteration() {
        let mut world = World::alloc();
        world.spawn((Velocity { x: 1.0, y: 2.0 },));
        world.spawn((Velocity { x: 3.0, y: 4.0 },));

        let query = world.query::<ReadVelocity>();
        let mut sum = 0.0;
        for item in query.iter() {
            sum += item.velocity.x + item.velocity.y;
        }

        assert_eq!(sum, 10.0);
    }

    #[test]
    fn derive_query_data_supports_single_w_lifetime_mutable_fetch_wrappers() {
        let mut world = World::alloc();
        let entity = world
            .spawn((Position { x: 0.0, y: 1.0 }, Velocity { x: 2.0, y: 3.0 }))
            .entity();

        let mut query = world.query::<MoveData>();
        for mut item in query.iter_mut() {
            item.position.x += item.velocity.x;
            item.position.y += item.velocity.y;
        }

        let updated = world.get::<Position>(entity).unwrap();
        assert_eq!(updated.x, 2.0);
        assert_eq!(updated.y, 4.0);
    }
}
