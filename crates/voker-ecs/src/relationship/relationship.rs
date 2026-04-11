#![expect(clippy::module_inception, reason = "For better structure.")]

use core::ops::DerefMut;

use super::RelationshipSourceSet;
use crate::component::Component;
use crate::entity::Entity;
use crate::prelude::HookContext;
use crate::utils::DebugName;
use crate::world::{DeferredWorld, EntityOwned};

pub type SourceIter<'a, T> =
    <<T as RelationshipTarget>::SourceSet as RelationshipSourceSet>::SourceIter<'a>;

pub trait Relationship: Component + Sized {
    type RelationshipTarget: RelationshipTarget<Relationship = Self>;

    const TARGET_FIELD_OFFSET: usize;

    const ALLOW_SELF_REFERENTIAL: bool = false;

    /// Gets the [`Entity`] of the linked entity.
    fn related_target(&self) -> Entity;

    /// Creates this [`Relationship`] from the given `entity`.
    fn from_target(entity: Entity) -> Self;

    fn raw_target_mut(this: &mut Self) -> &mut Entity;

    fn on_insert(mut world: DeferredWorld, context: HookContext) {
        let entity = context.entity;
        // Hook is called before operation, the entities and components should exist.
        let entity_ref = world.entity_ref(entity);
        let target_entity = entity_ref.get::<Self>().unwrap().related_target();

        if !Self::ALLOW_SELF_REFERENTIAL && target_entity == entity {
            voker_utils::cold_path();
            log::warn!(
                "{}The {}({target_entity:?}) relationship on entity {entity:?} points to itself. This invalid\
                relationship has been removed.\nIf this is intended behavior self-referential relations can\
                be enabled with the allow_self_referential attribute: #[relationship(allow_self_referential)]",
                context.caller,
                DebugName::type_name::<Self>(),
            );
            world.commands().with_entity(entity).remove::<Self>();
            return;
        }

        // For one-to-one link, remove existing relationship before adding new one
        // For one-to-many link, no negative effects.
        if <<Self::RelationshipTarget as RelationshipTarget>::SourceSet as RelationshipSourceSet>::SINGLE_ENTITY {
            let target_entity_ref = world.get_entity_ref(target_entity).ok();
            let current_source_to_remove = target_entity_ref
                .as_ref()
                .and_then(|target_entity_ref| target_entity_ref.get::<Self::RelationshipTarget>())
                .and_then(|target| target.related_sources().remove_before_insert());

            if let Some(current_source) = current_source_to_remove {
                world.commands().with_entity(current_source).try_remove::<Self>();
            }
        }

        if let Ok(mut entity_commands) = world.commands().try_with_entity(target_entity) {
            entity_commands.push(move |mut owned: EntityOwned| {
                if let Some(target) = owned.get_mut::<Self::RelationshipTarget>() {
                    RelationshipTarget::raw_sources_mut(target.into_inner()).insert(entity);
                } else {
                    let mut target = Self::RelationshipTarget::with_hint(1);
                    RelationshipTarget::raw_sources_mut(&mut target).insert(entity);
                    owned.insert::<Self::RelationshipTarget>(target);
                }
            });
        } else {
            voker_utils::cold_path();
            log::warn!(
                "{}The {}({target_entity:?}) linked on entity {entity:?} relates to an entity\
                that does not exist. This invalid link has been removed.",
                context.caller,
                DebugName::type_name::<Self>(),
            );
            world.commands().with_entity(entity).try_remove::<Self>();
        }
    }

    fn on_discard(mut world: DeferredWorld, context: HookContext) {
        let entity = context.entity;
        // Hook is called before operation, the entities and components should exist.
        let entity_ref = world.entity_ref(entity);
        let target_entity = entity_ref.get::<Self>().unwrap().related_target();

        if let Ok(mut target_mut) = world.get_entity_mut(target_entity)
            && let Some(mut target) = target_mut.get_mut::<Self::RelationshipTarget>()
        {
            RelationshipTarget::raw_sources_mut(target.deref_mut()).remove(entity);
            if target.is_empty() {
                world.commands().with_entity(target_entity).push_silenced(
                    move |mut e: EntityOwned| {
                        if e.get::<Self::RelationshipTarget>()
                            .is_some_and(RelationshipTarget::is_empty)
                        {
                            e.remove::<Self::RelationshipTarget>();
                        }
                    },
                );
            }
        }
    }
}

pub trait RelationshipTarget: Component + Sized {
    type Relationship: Relationship<RelationshipTarget = Self>;
    type SourceSet: RelationshipSourceSet;

    const LINKED_LIFECYCLE: bool = false;

    fn related_sources(&self) -> &Self::SourceSet;

    fn from_sources(sources: Self::SourceSet) -> Self;

    fn raw_sources_mut(this: &mut Self) -> &mut Self::SourceSet;

    fn with_hint(size_hint: usize) -> Self {
        Self::from_sources(<Self::SourceSet as RelationshipSourceSet>::with_hint(
            size_hint,
        ))
    }

    fn on_discard(mut world: DeferredWorld, context: HookContext) {
        let (entities, mut commands) = world.entities_and_commands();
        let entity = context.entity;

        // Hook is called before operation, the entities and components should exist.
        let entity_ref = entities.get_ref(entity).unwrap();
        let target_ref = entity_ref.get::<Self>().unwrap();

        for source_entity in target_ref.iter() {
            commands.with_entity(source_entity).try_remove::<Self::Relationship>();
        }
    }

    fn on_despawn(mut world: DeferredWorld, context: HookContext) {
        if Self::LINKED_LIFECYCLE {
            let (entities, mut cmd) = world.entities_and_commands();
            let entity = context.entity;

            // Hook is called before operation, the entities and components should exist.
            let entity_ref = entities.get_ref(entity).unwrap();
            let target_ref = entity_ref.get::<Self>().unwrap();
            for source_entity in target_ref.iter() {
                cmd.try_despawn(source_entity);
            }
        }
    }

    #[inline]
    fn iter(&self) -> SourceIter<'_, Self> {
        self.related_sources().iter()
    }

    /// Returns the number of entities in this collection.
    #[inline]
    fn len(&self) -> usize {
        self.related_sources().len()
    }

    /// Returns true if this entity collection is empty.
    #[inline]
    fn is_empty(&self) -> bool {
        self.related_sources().is_empty()
    }
}
