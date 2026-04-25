//! Generic entity relationship model for hierarchies and custom graph data.
//!
//! A relationship is represented by two cooperating components:
//!
//! | Component | Location | Role |
//! |---|---|---|
//! | [`Relationship`] | source entity | Points to one target entity |
//! | [`RelationshipTarget`] | target entity | Caches all source entities |
//!
//! Component hooks keep both sides synchronized automatically. User code
//! typically only inserts/removes the source-side component.
//!
//! # Lifecycle
//!
//! | Event | Effect |
//! |---|---|
//! | Insert source `R` on entity `S` | Adds `S` to `R::RelationshipTarget` on the target |
//! | Remove source `R` from entity `S` | Removes `S` from the target cache; removes the cache if it becomes empty |
//! | Remove target cache `RT` from entity `T` | Removes `R` from every cached source |
//! | Despawn target `T` (when `LINKED_LIFECYCLE = true`) | Despawns every cached source |
//!
//! # Storage
//!
//! The target-side cache stores its source entities in a [`RelationshipSourceSet`]. The built-in
//! implementations cover common use cases:
//!
//! | Type | Cardinality | Order |
//! |---|---|---|
//! | `Entity` | one-to-one | — |
//! | `Option<Entity>` | one-to-one (optional) | — |
//! | `Vec<Entity>` | one-to-many | insertion |
//! | `VecDeque<Entity>` | one-to-many | insertion |
//! | `SmallVec<Entity, N>` | one-to-many | insertion |
//! | `BTreeSet<Entity>` | one-to-many | sorted |
//! | `EntityHashSet` | one-to-many | unordered |
//! | `EntityIndexSet` | one-to-many | insertion |
//!
//! # Query helpers
//!
//! [`Query`] is extended with relationship-aware traversal methods when the query data matches
//! either a [`Relationship`] or a [`RelationshipTarget`]:
//!
//! | Method | Description |
//! |---|---|
//! | [`Query::related`] | Gets the single target of a source relationship |
//! | [`Query::relationship_sources`] | Iterates all sources pointing to a target |
//! | [`Query::root_ancestor`] | Walks upward to find the root of the hierarchy |
//! | [`Query::iter_ancestors`] | Iterates all ancestors bottom-up |
//! | [`Query::iter_descendants`] | Breadth-first descendant traversal |
//! | [`Query::iter_descendants_depth_first`] | Depth-first descendant traversal |
//! | [`Query::iter_leaves`] | All leaf descendants (no further children) |
//! | [`Query::iter_siblings`] | Entities sharing the same relationship target |
//!
//! # Mutation APIs
//!
//! - [`EntityOwned`]-based immediate mutations: `with_related`, `add_related`, `remove_related`,
//!   `despawn_related`, `insert_recursive`, `remove_recursive`.
//! - [`EntityCommands`]-based deferred mutations: command-queue equivalents of the above.
//! - [`RelatedSpawner`] and [`RelatedSpawnerCommands`]: ergonomic spawning helpers that
//!   automatically link new entities into a relationship.
//!
//! [`Query::related`]: crate::query::Query::related
//! [`Query::relationship_sources`]: crate::query::Query::relationship_sources
//! [`Query::root_ancestor`]: crate::query::Query::root_ancestor
//! [`Query::iter_ancestors`]: crate::query::Query::iter_ancestors
//! [`Query::iter_descendants`]: crate::query::Query::iter_descendants
//! [`Query::iter_descendants_depth_first`]: crate::query::Query::iter_descendants_depth_first
//! [`Query::iter_leaves`]: crate::query::Query::iter_leaves
//! [`Query::iter_siblings`]: crate::query::Query::iter_siblings
//! [`EntityOwned`]: crate::world::EntityOwned
//! [`EntityCommands`]: crate::command::EntityCommands

mod accessor;
mod collection;
mod methods;
mod query;
mod relationship;

pub use accessor::{RelationshipAccessor, RelationshipRegistrar};
pub use collection::{OrderedRelationshipSourceSet, RelationshipSourceSet};
pub use methods::{RelatedSpawner, RelatedSpawnerCommands};
pub use query::{AncestorIter, DescendantDepthFirstIter, DescendantIter};
pub use relationship::{Relationship, RelationshipTarget, SourceIter};
