//! Generic entity relationship model used by hierarchy and custom graph data.
//!
//! A relationship is represented by two cooperating components:
//! - [`Relationship`]: stored on a source entity, pointing to one target entity.
//! - [`RelationshipTarget`]: stored on the target entity, caching all source entities.
//!
//! The runtime keeps both sides in sync through component hooks. Most users edit
//! only the source component and treat the target-side cache as derived data.
//!
//! # Lifecycle model
//!
//! - Inserting/updating a source relationship updates the target cache.
//! - Removing a source relationship detaches the source from the previous target.
//! - Removing a target cache detaches all sources that point to it.
//! - If [`RelationshipTarget::LINKED_LIFECYCLE`] is enabled, despawning a target
//!   will also despawn all linked sources.
//!
//! # Main APIs
//!
//! - [`EntityOwned`]-based immediate mutations: relation insert/remove/recursive
//!   traversal helpers.
//! - [`EntityCommands`]-based deferred mutations: command-queue equivalents.
//! - [`RelatedSpawner`] and [`RelatedSpawnerCommands`]: ergonomic related entity
//!   spawning helpers.
//!
//! [`EntityOwned`]: crate::world::EntityOwned
//! [`EntityCommands`]: crate::command::EntityCommands

mod accessor;
mod collection;
mod methods;
mod relationship;

pub use accessor::{RelationshipAccessor, RelationshipRegistrar};
pub use collection::{OrderedRelationshipSourceSet, RelationshipSourceSet};
pub use methods::{RelatedSpawner, RelatedSpawnerCommands};
pub use relationship::{Relationship, RelationshipTarget};
