#![expect(clippy::module_inception, reason = "For better structure.")]

use core::ops::DerefMut;

use super::LinkSourceSet;
use crate::component::Component;
use crate::entity::Entity;
use crate::prelude::HookContext;
use crate::utils::DebugName;
use crate::world::{DeferredWorld, EntityOwned};

pub type SourceIter<'a, T> = <<T as LinkTarget>::SourceSet as LinkSourceSet>::SourceIter<'a>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkHookMode {
    Run,
    Skip,
    RunIfNotLinkedSpawn,
}

pub trait LinkSource: Component + Sized {
    type Target: LinkTarget<Source = Self>;

    const LINKED_SPAWN: bool = false;
    const ALLOW_ONE_TO_ONE: bool = true;
    const ALLOW_SELF_LINKING: bool = false;

    /// Gets the [`Entity`] of the linked entity.
    fn linked_target(&self) -> Entity;

    /// Creates this [`LinkSource`] from the given `entity`.
    fn from_target(entity: Entity) -> Self;

    fn set_target_risky(this: &mut Self, entity: Entity);

    fn on_insert(mut world: DeferredWorld, context: HookContext) {
        match context.link_hook_mode {
            LinkHookMode::Skip => return,
            LinkHookMode::RunIfNotLinkedSpawn if Self::LINKED_SPAWN => return,
            _ => {}
        }

        let entity = context.entity;
        // Hook is called before operation, the entities and components should exist.
        let entity_ref = world.entity_ref(entity);
        let target_entity = entity_ref.get::<Self>().unwrap().linked_target();

        if !Self::ALLOW_SELF_LINKING && target_entity == entity {
            voker_utils::cold_path();
            log::warn!(
                "{} The {}({target_entity:?}) linked on entity `{entity:?}` points to itself.\
                This invalid link has been removed.\nIf this is intended behavior self-linking\
                can be enabled with the `ALLOW_SELF_LINKING` by #[link_source(self_linking)]",
                context.caller,
                DebugName::type_name::<Self>(),
            );
            world.commands().with_entity(entity).remove::<Self>();
            return;
        }

        if Self::ALLOW_ONE_TO_ONE {
            // For one-to-one link, remove existing relationship before adding new one
            // For one-to-many link, no negative effects.
            let target_entity_ref = world.get_entity_ref(target_entity).ok();
            let current_source_to_remove = target_entity_ref
                .as_ref()
                .and_then(|target_entity_ref| target_entity_ref.get::<Self::Target>())
                .and_then(|link_target| link_target.linked_sources().remove_before_insert());

            if let Some(current_source) = current_source_to_remove {
                world.commands().with_entity(current_source).try_remove::<Self>();
            }
        }

        if let Ok(mut entity_commands) = world.commands().try_with_entity(target_entity) {
            entity_commands.push(move |mut owned: EntityOwned| {
                if let Some(target) = owned.get_mut::<Self::Target>() {
                    LinkTarget::sources_mut_risky(target.into_inner()).insert(entity);
                } else {
                    let mut target = Self::Target::with_hint(1);
                    LinkTarget::sources_mut_risky(&mut target).insert(entity);
                    owned.insert::<Self::Target>(target);
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
            world.commands().with_entity(entity).remove::<Self>();
        }
    }

    fn on_discard(mut world: DeferredWorld, context: HookContext) {
        match context.link_hook_mode {
            LinkHookMode::Skip => return,
            LinkHookMode::RunIfNotLinkedSpawn if Self::LINKED_SPAWN => return,
            _ => {}
        }

        let entity = context.entity;
        // Hook is called before operation, the entities and components should exist.
        let entity_ref = world.entity_ref(entity);
        let target_entity = entity_ref.get::<Self>().unwrap().linked_target();

        if let Ok(mut target_entity_mut) = world.get_entity_mut(target_entity)
            && let Some(mut link_target) = target_entity_mut.get_mut::<Self::Target>()
        {
            LinkTarget::sources_mut_risky(link_target.deref_mut()).remove(entity);
            if link_target.is_empty() {
                world.commands().with_entity(target_entity).push_silenced(
                    move |mut e: EntityOwned| {
                        if e.get::<Self::Target>().is_some_and(LinkTarget::is_empty) {
                            e.remove::<Self::Target>();
                        }
                    },
                );
            }
        }
    }
}

pub trait LinkTarget: Component + Sized {
    type Source: LinkSource<Target = Self>;
    type SourceSet: LinkSourceSet;

    fn linked_sources(&self) -> &Self::SourceSet;

    fn sources_mut_risky(this: &mut Self) -> &mut Self::SourceSet;

    fn from_sources(sources: Self::SourceSet) -> Self;

    fn with_hint(size_hint: usize) -> Self {
        Self::from_sources(<Self::SourceSet as LinkSourceSet>::with_hint(size_hint))
    }

    fn on_discard(mut world: DeferredWorld, context: HookContext) {
        if context.link_hook_mode != LinkHookMode::Run {
            // For LinkTarget we don't want to run this hook even if
            // it isn't linked, but for LinkSource we do.
            return;
        }
        let (entities, mut commands) = world.entities_and_commands();
        let entity = context.entity;

        // Hook is called before operation, the entities and components should exist.
        let entity_ref = entities.get_ref(entity).unwrap();
        let target_ref = entity_ref.get::<Self>().unwrap();

        for source_entity in target_ref.iter() {
            commands.with_entity(source_entity).try_remove::<Self::Source>();
        }
    }

    fn on_despawn(mut world: DeferredWorld, context: HookContext) {
        let (entities, mut commands) = world.entities_and_commands();
        let entity = context.entity;

        // Hook is called before operation, the entities and components should exist.
        let entity_ref = entities.get_ref(entity).unwrap();
        let target_ref = entity_ref.get::<Self>().unwrap();
        for source_entity in target_ref.iter() {
            commands.try_despawn(source_entity);
        }
    }

    #[inline]
    fn iter(&self) -> SourceIter<'_, Self> {
        self.linked_sources().iter()
    }

    /// Returns the number of entities in this collection.
    #[inline]
    fn len(&self) -> usize {
        self.linked_sources().len()
    }

    /// Returns true if this entity collection is empty.
    #[inline]
    fn is_empty(&self) -> bool {
        self.linked_sources().is_empty()
    }
}
