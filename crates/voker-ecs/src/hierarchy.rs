//! Canonical parent-child hierarchy built on top of `Relationship`.
//!
//! `ChildOf` is the source-of-truth relationship stored on the child entity.
//! `Children` is the reverse cached relationship target stored on the parent entity.
//! Keep hierarchy edits on `ChildOf`; `Children` is maintained by hooks.
//!
//! # Lifecycle semantics
//!
//! - Inserting/replacing `ChildOf(parent)` updates the old/new parent `Children`
//!   cache immediately through hooks.
//! - Removing `ChildOf` detaches the child from its parent.
//! - With `linked_lifecycle` on [`Children`], despawning a parent recursively
//!   despawns all descendants.
//!
//! # Recursive operations
//!
//! [`EntityOwned::insert_recursive`] and [`EntityOwned::remove_recursive`] can
//! traverse hierarchy trees through `Children`. Traversal does not detect cycles;
//! use these APIs only on acyclic graphs.
//!
//! # Complexity
//!
//! Single edge attach/detach is $O(1)$ plus source-set insertion/removal cost.
//! Recursive operations are $O(n)$ in the number of traversed descendants.
//!
//! [`EntityOwned::insert_recursive`]: crate::world::EntityOwned::insert_recursive
//! [`EntityOwned::remove_recursive`]: crate::world::EntityOwned::remove_recursive

use alloc::vec::Vec;
use core::ops::Deref;
use core::slice;

use serde::{Deserialize, Serialize};
use voker_reflect::Reflect;

use crate::bundle::Bundle;
use crate::clone::{CloneContext, CloneSource, CloneTarget};
use crate::command::EntityCommands;
use crate::component::Component;
use crate::entity::Entity;
use crate::reflect::{ReflectComponent, ReflectFromWorld};
use crate::relationship::{RelatedSpawner, RelatedSpawnerCommands};
use crate::world::{EntityOwned, FromWorld, World};

/// Stores the parent entity of this child entity.
///
/// This relationship powers the canonical hierarchy in `voker-ecs`.
///
/// When inserted or updated, `Children` is synchronized immediately via hooks.
/// When removed, the child is detached from the previous parent.
///
/// With `linked_lifecycle = true`, despawning a parent recursively despawns children.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[derive(Reflect, Component, Serialize, Deserialize)]
#[reflect(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[type_data(ReflectComponent, ReflectFromWorld)]
#[relationship(relationship_target = Children)]
pub struct ChildOf(#[related] pub Entity);

impl ChildOf {
    /// Returns the parent entity.
    #[inline]
    pub fn parent(&self) -> Entity {
        self.0
    }
}

impl FromWorld for ChildOf {
    fn from_world(_: &mut World) -> Self {
        Self(Entity::PLACEHOLDER)
    }
}

/// Reverse cache of child entities targeting this parent via `ChildOf`.
///
/// This component is maintained by relationship hooks and should be treated
/// as read-only view data.
///
/// Avoid mutating this component directly. Update `ChildOf` on child entities
/// instead so both sides of the relationship remain synchronized.
#[derive(Reflect, Component, Default, Debug, PartialEq, Eq)]
#[component(cloner = Children::cloner)]
#[reflect(Default, Debug, PartialEq)]
#[type_data(ReflectComponent, ReflectFromWorld)]
#[relationship_target(relationship = ChildOf, linked_lifecycle)]
pub struct Children(#[related] Vec<Entity>);

impl Children {
    /// A custom component cloner.
    ///
    /// When no custom cloner is specified, the macro falls back to [`ComponentCloner::relationship_target`].
    /// This implementation is almost identical to this function, but requires the type to be Clone. Since
    /// we don't want Children to be clonable, we provide a cloner manually.
    ///
    /// [`ComponentCloner::relationship_target`]: crate::clone::ComponentCloner::relationship_target
    fn cloner(src: CloneSource, mut dst: CloneTarget, ctx: &mut CloneContext) {
        if ctx.linked_clone() {
            dst.write::<Self>(Self(src.read::<Self>().0.clone()));
            ctx.defer_map_entities::<Self>();
        } else {
            dst.write::<Self>(Self(Vec::new()));
        }
    }
}

impl<'a> IntoIterator for &'a Children {
    type Item = <Self::IntoIter as Iterator>::Item;
    type IntoIter = slice::Iter<'a, Entity>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl Deref for Children {
    type Target = [Entity];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'w> EntityOwned<'w> {
    /// Spawns a child entity related to this entity with [`ChildOf`].
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn with_child(&mut self, bundle: impl Bundle) -> &mut Self {
        self.with_related::<ChildOf>(bundle)
    }

    /// Spawns child entities related to this entity with [`ChildOf`]
    /// by running a builder closure against a [`RelatedSpawner`].
    #[inline]
    pub fn with_children(
        &mut self,
        func: impl FnOnce(&mut RelatedSpawner<'_, ChildOf>),
    ) -> &mut Self {
        let target = self.entity();
        self.world_scope(|world| {
            func(&mut RelatedSpawner::new(world, target));
        });
        self
    }

    /// Adds one child to this entity via [`ChildOf`].
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn add_child(&mut self, child: Entity) -> &mut Self {
        self.add_related::<ChildOf>(child)
    }

    /// Adds many children to this entity via [`ChildOf`].
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn add_children(&mut self, children: &[Entity]) -> &mut Self {
        self.insert_related::<ChildOf>(children)
    }

    /// Removes one child relationship from this entity.
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn remove_child(&mut self, child: Entity) -> &mut Self {
        self.remove_related::<ChildOf>(&[child])
    }

    /// Removes specific child relationships from this entity.
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn remove_children(&mut self, children: &[Entity]) -> &mut Self {
        self.remove_related::<ChildOf>(children)
    }

    /// Removes all child relationships from this entity.
    ///
    /// This detaches children but does not despawn child entities.
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn detach_children(&mut self) -> &mut Self {
        self.detach_related::<Children>()
    }

    /// Despawns all children of this entity.
    ///
    /// This removes child entities entirely (and recursively, because
    /// `Children` enables linked lifecycle).
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn despawn_children(&mut self) -> &mut Self {
        self.despawn_related::<Children>()
    }
}

impl<'a> EntityCommands<'a> {
    /// Spawns a child entity related to this entity with [`ChildOf`].
    #[inline]
    pub fn with_child(&mut self, bundle: impl Bundle) -> &mut Self {
        self.with_related::<ChildOf>(bundle)
    }

    /// Spawns child entities related to this entity with [`ChildOf`]
    /// by running a builder closure against a [`RelatedSpawnerCommands`].
    #[inline]
    pub fn with_children(
        &mut self,
        func: impl FnOnce(&mut RelatedSpawnerCommands<'_, ChildOf>),
    ) -> &mut Self {
        self.with_related_entities::<ChildOf>(func)
    }

    /// Adds one child to this entity via [`ChildOf`].
    #[inline]
    pub fn add_child(&mut self, child: Entity) -> &mut Self {
        self.add_related::<ChildOf>(child)
    }

    /// Adds many children to this entity via [`ChildOf`].
    #[inline]
    pub fn add_children(&mut self, children: &[Entity]) -> &mut Self {
        self.insert_related::<ChildOf>(children)
    }

    /// Removes specific child relationships from this entity.
    #[inline]
    pub fn remove_children(&mut self, children: &[Entity]) -> &mut Self {
        self.remove_related::<ChildOf>(children)
    }

    /// Removes all child relationships from this entity.
    ///
    /// This detaches children but does not despawn child entities.
    #[inline]
    pub fn detach_all_children(&mut self) -> &mut Self {
        self.detach_all_related::<ChildOf>()
    }

    /// Despawns all children of this entity.
    ///
    /// This removes child entities entirely (and recursively, because
    /// `Children` enables linked lifecycle).
    #[inline]
    pub fn despawn_all_children(&mut self) -> &mut Self {
        self.despawn_all_related::<ChildOf>()
    }
}

#[cfg(test)]
mod tests {
    use crate::hierarchy::{ChildOf, Children};
    use crate::prelude::*;

    #[test]
    fn insert_childof_updates_children() {
        let mut world = World::alloc();

        let parent = world.spawn(()).entity();
        let child = world.spawn(ChildOf(parent)).entity();

        let children = world.get::<Children>(parent).unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0], child);
    }

    #[test]
    fn replacing_parent_updates_both_sides() {
        let mut world = World::alloc();

        let parent_a = world.spawn(()).entity();
        let parent_b = world.spawn(()).entity();
        let child = world.spawn(ChildOf(parent_a)).entity();

        world.entity_owned(child).insert(ChildOf(parent_b));

        assert!(
            world
                .get::<Children>(parent_a)
                .is_none_or(|children| children.is_empty())
        );

        let children_b = world.get::<Children>(parent_b).unwrap();
        assert_eq!(children_b.len(), 1);
        assert_eq!(children_b[0], child);
    }

    #[test]
    fn removing_childof_detaches_from_parent() {
        let mut world = World::alloc();

        let parent = world.spawn(()).entity();
        let child = world.spawn(ChildOf(parent)).entity();

        world.entity_owned(child).remove::<ChildOf>();

        assert!(
            world
                .get::<Children>(parent)
                .is_none_or(|children| children.is_empty())
        );
    }

    #[test]
    fn despawn_parent_recursively_despawns_children() {
        let mut world = World::alloc();

        let root = world.spawn(()).entity();
        let child = world.spawn(ChildOf(root)).entity();
        let grandchild = world.spawn(ChildOf(child)).entity();

        world.despawn(root).unwrap();

        assert!(world.get_entity_ref(root).is_err());
        assert!(world.get_entity_ref(child).is_err());
        assert!(world.get_entity_ref(grandchild).is_err());
    }

    #[test]
    fn entity_owned_children_convenience_methods() {
        let mut world = World::alloc();

        let root = world.spawn(()).entity();
        let a = world.spawn(()).entity();
        let b = world.spawn(()).entity();

        world
            .entity_owned(root)
            .add_child(a)
            .add_children(&[b])
            .with_child(())
            .with_children(|children| {
                children.spawn(());
            });

        let children = world.get::<Children>(root).unwrap().to_vec();

        assert!(children.contains(&a));
        assert!(children.contains(&b));
        assert!(children.len() >= 4);

        world.entity_owned(root).remove_child(a);
        assert!(!world.get::<Children>(root).unwrap().contains(&a));
    }

    #[test]
    fn entity_commands_children_convenience_methods() {
        let mut world = World::alloc();

        let root = world.spawn(()).entity();
        let a = world.spawn(()).entity();
        let b = world.spawn(()).entity();

        world
            .commands()
            .with_entity(root)
            .add_child(a)
            .add_children(&[b])
            .with_child(())
            .with_children(|children| {
                children.spawn(());
            });
        world.flush();

        let children = world.get::<Children>(root).unwrap();
        assert!(children.contains(&a));
        assert!(children.contains(&b));
        assert!(children.len() >= 4);

        world.commands().with_entity(root).remove_children(&[a]);
        world.flush();
        assert!(!world.get::<Children>(root).unwrap().contains(&a));
    }
}
