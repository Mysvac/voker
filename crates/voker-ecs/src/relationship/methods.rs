use alloc::boxed::Box;
use alloc::vec::Vec;
use core::marker::PhantomData;

use crate::bundle::Bundle;
use crate::command::{Commands, EntityCommands};
use crate::entity::Entity;
use crate::relationship::RelationshipTarget;
use crate::world::World;
use crate::{utils::DebugLocation, world::EntityOwned};

use super::Relationship;

impl<'w> EntityOwned<'w> {
    /// Spawns one source entity linked to `self` through relationship `R`.
    ///
    /// The spawned entity receives `bundle` and `R::from_target(self.entity())`.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn with_related<R: Relationship>(&mut self, bundle: impl Bundle) -> &mut Self {
        let this = self.entity();
        let caller = DebugLocation::caller();

        self.world_scope(|world| {
            world.spawn_with_caller((bundle, R::from_target(this)), caller);
        });

        self
    }

    /// Spawns source entities linked to `self` through relationship `R`.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn with_related_entities<R: Relationship>(
        &mut self,
        func: impl FnOnce(&mut RelatedSpawner<R>),
    ) -> &mut Self {
        let this = self.entity();

        self.world_scope(|world| {
            func(&mut RelatedSpawner::new(world, this));
        });

        self
    }

    /// Adds one existing source entity to this target through `R`.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn add_related<R: Relationship>(&mut self, entity: Entity) -> &mut Self {
        self.add_relateds::<R>(&[entity])
    }

    /// Adds multiple existing source entities to this target through `R`.
    ///
    /// Existing `R` components are retargeted in place when possible.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn add_relateds<R: Relationship>(&mut self, entities: &[Entity]) -> &mut Self {
        let this = self.entity();
        let caller = DebugLocation::caller();

        self.world_scope(|world| {
            for &source in entities {
                let mut e = world.entity_owned(source);
                modify_or_insert_relationship_with_caller::<R>(&mut e, this, caller);
            }
        });
        self
    }

    /// Removes `R` links from the provided source entities if they point to `self`.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn remove_related<R: Relationship>(&mut self, entities: &[Entity]) -> &mut Self {
        let this = self.entity();
        let caller = DebugLocation::caller();

        self.world_scope(|world| {
            for &source in entities {
                let mut entity_owned = world.entity_owned(source);
                if entity_owned.get::<R>().is_some_and(|l| l.related_target() == this) {
                    entity_owned.remove_explicit_with_caller::<R>(caller);
                }
            }
        });

        self
    }

    /// Detaches all source entities tracked by target cache `R` from this entity.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn detach_related<R: RelationshipTarget>(&mut self) -> &mut Self {
        let caller = DebugLocation::caller();
        self.remove_explicit_with_caller::<R>(caller);
        self
    }

    /// Despawns all source entities tracked by target cache `R`.
    ///
    /// Source entity IDs are collected first so hooks/observers can still read
    /// the relationship cache consistently during command processing.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn despawn_related<R: RelationshipTarget>(&mut self) -> &mut Self {
        let caller = DebugLocation::caller();
        if let Some(relationship_target) = self.get::<R>() {
            let sources: Vec<Entity> = relationship_target.iter().collect();
            self.world_scope(|world| {
                // Deleting from the end keeps Vec removal O(1) per entity.
                for &entity in sources.iter().rev() {
                    let _ = world.despawn_with_caller(entity, caller);
                }
            });
        }
        self
    }

    /// Detaches all sources linked through relationship `L`.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn detach_all_related<R: Relationship>(&mut self) -> &mut Self {
        self.detach_related::<R::RelationshipTarget>()
    }

    /// Despawns all sources linked through relationship `L`.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn despawn_all_related<R: Relationship>(&mut self) -> &mut Self {
        self.despawn_related::<R::RelationshipTarget>()
    }

    /// Inserts `bundle` on this entity and recursively on all linked sources.
    ///
    /// Traversal follows `L` edges from target to sources. Cycles are not
    /// detected and can lead to infinite recursion.
    pub fn insert_recursive<R: RelationshipTarget>(
        &mut self,
        bundle: impl Bundle + Clone,
    ) -> &mut Self {
        self.insert(bundle.clone());

        if let Some(relationship_target) = self.get::<R>() {
            let sources: Vec<Entity> = relationship_target.iter().collect();
            for source in sources.iter().copied() {
                self.world_scope(|world| {
                    world.entity_owned(source).insert_recursive::<R>(bundle.clone());
                });
            }
        }

        self
    }

    /// Removes bundle `B` from this entity and recursively from all linked sources.
    ///
    /// Traversal follows `L` edges from target to sources. Cycles are not
    /// detected and can lead to infinite recursion.
    pub fn remove_recursive<R: RelationshipTarget, B: Bundle>(&mut self) -> &mut Self {
        self.remove::<B>();

        if let Some(relationship_target) = self.get::<R>() {
            let sources: Vec<Entity> = relationship_target.iter().collect();
            for source in sources.iter().copied() {
                self.world_scope(|world| {
                    world.entity_owned(source).remove_recursive::<R, B>();
                });
            }
        }

        self
    }
}

fn modify_or_insert_relationship_with_caller<R: Relationship>(
    this: &mut EntityOwned,
    target_entity: Entity,
    caller: DebugLocation,
) {
    if size_of::<R>() > size_of::<Entity>() {
        this.assert_is_spawned_with_caller(caller);
        let source_entity = this.entity();

        let modified = this.world_scope(|world| {
            let modified = world
                .modify_component_with_caller::<R, _>(source_entity, caller, |r| {
                    *Relationship::raw_target_mut(r) = target_entity;
                })
                .expect("entity access must be valid")
                .is_some();

            world.flush();

            modified
        });

        if modified {
            return;
        }
    }

    this.insert_with_caller(R::from_target(target_entity), caller);
}

impl<'a> EntityCommands<'a> {
    /// Queues spawning one source entity linked to this target through `R`.
    pub fn with_related<R: Relationship>(&mut self, bundle: impl Bundle) -> &mut Self {
        let target = self.entity();
        self.commands().spawn((bundle, R::from_target(target)));
        self
    }

    /// Queues spawning multiple source entities through a spawner callback.
    ///
    /// The callback receives a [`RelatedSpawnerCommands`] bound to this target.
    pub fn with_related_entities<R: Relationship>(
        &mut self,
        func: impl FnOnce(&mut RelatedSpawnerCommands<R>),
    ) -> &mut Self {
        let target = self.entity();
        func(&mut RelatedSpawnerCommands::new(self.commands(), target));
        self
    }

    /// Relates the given entity to this with the relation `R`.
    ///
    /// See [`add_related`](Self::add_related) if you want to relate more than one entity.
    pub fn add_related<R: Relationship>(&mut self, entity: Entity) -> &mut Self {
        self.queue(move |mut entity_owned: EntityOwned| {
            entity_owned.add_relateds::<R>(&[entity]);
        })
    }

    /// Relates the given entities to this entity with the relation `R`.
    ///
    /// See [`add_related`](Self::add_related) if you want to relate only one entity.
    pub fn add_relateds<R: Relationship>(&mut self, entities: &[Entity]) -> &mut Self {
        let entities: Box<[Entity]> = entities.into();

        self.queue(move |mut entity: EntityOwned| {
            entity.add_relateds::<R>(&entities);
        })
    }

    /// Removes the relation `R` between this entity and all its related entities.
    pub fn detach_all_related<R: Relationship>(&mut self) -> &mut Self {
        self.queue(|mut entity: EntityOwned| {
            entity.detach_all_related::<R>();
        })
    }

    /// Removes all related source entities linked through `R` by despawning them.
    pub fn despawn_all_related<R: Relationship>(&mut self) -> &mut Self {
        self.queue(|mut entity: EntityOwned| {
            entity.despawn_all_related::<R>();
        })
    }

    /// Removes the relation `R` between this entity and the given entities.
    pub fn remove_related<R: Relationship>(&mut self, related: &[Entity]) -> &mut Self {
        let related: Box<[Entity]> = related.into();

        self.queue(move |mut entity: EntityOwned| {
            entity.remove_related::<R>(&related);
        })
    }

    /// Removes the target cache component `R` from this entity, detaching sources.
    pub fn detach_related<R: RelationshipTarget>(&mut self) -> &mut Self {
        self.queue(move |mut entity: EntityOwned| {
            entity.detach_related::<R>();
        })
    }

    /// Despawns source entities listed by target cache `R`.
    pub fn despawn_related<R: RelationshipTarget>(&mut self) -> &mut Self {
        self.queue(move |mut entity: EntityOwned| {
            entity.despawn_related::<R>();
        })
    }

    /// Inserts a component or bundle of components into the entity and all related entities,
    /// traversing the relationship tracked in `S` in a breadth-first manner.
    ///
    /// # Warning
    ///
    /// This method should only be called on relationships that form a tree-like structure.
    /// Any cycles will cause this method to loop infinitely.
    pub fn insert_recursive<R: RelationshipTarget>(
        &mut self,
        bundle: impl Bundle + Clone,
    ) -> &mut Self {
        self.queue(move |mut entity: EntityOwned| {
            entity.insert_recursive::<R>(bundle);
        })
    }

    /// Removes a component or bundle of components of type `B` from the entity and all related entities,
    /// traversing the relationship tracked in `S` in a breadth-first manner.
    ///
    /// # Warning
    ///
    /// This method should only be called on relationships that form a tree-like structure.
    /// Any cycles will cause this method to loop infinitely.
    pub fn remove_recursive<R: RelationshipTarget, B: Bundle>(&mut self) -> &mut Self {
        self.queue(move |mut entity: EntityOwned| {
            entity.remove_recursive::<R, B>();
        })
    }
}

pub struct RelatedSpawner<'w, R: Relationship> {
    target: Entity,
    world: &'w mut World,
    _marker: PhantomData<R>,
}

impl<'w, R: Relationship> RelatedSpawner<'w, R> {
    /// Creates a new instance that will spawn entities targeting the `target` entity.
    pub fn new(world: &'w mut World, target: Entity) -> Self {
        Self {
            world,
            target,
            _marker: PhantomData,
        }
    }

    /// Returns a reborrowed spawner with a shorter lifetime.
    pub fn reborrow(&mut self) -> RelatedSpawner<'_, R> {
        RelatedSpawner {
            target: self.target,
            world: self.world,
            _marker: PhantomData,
        }
    }

    /// Spawns one source entity linked to the configured target.
    pub fn spawn(&mut self, bundle: impl Bundle) -> EntityOwned<'_> {
        self.world.spawn((R::from_target(self.target), bundle))
    }

    /// Spawns an entity with an `R` relationship targeting the `target`
    /// entity this spawner was initialized with.
    pub fn spawn_empty(&mut self) -> EntityOwned<'_> {
        self.world.spawn(R::from_target(self.target))
    }

    /// Returns the relationship target entity.
    pub fn target_entity(&self) -> Entity {
        self.target
    }

    /// Returns the world used by this spawner.
    pub fn world(&self) -> &World {
        self.world
    }

    /// Returns mutable access to the world used by this spawner.
    pub fn world_mut(&mut self) -> &mut World {
        self.world
    }
}

pub struct RelatedSpawnerCommands<'w, R: Relationship> {
    target: Entity,
    commands: Commands<'w, 'w>,
    _marker: PhantomData<R>,
}

impl<'w, R: Relationship> RelatedSpawnerCommands<'w, R> {
    /// Creates a new instance that will spawn entities targeting the `target` entity.
    pub fn new(commands: Commands<'w, 'w>, target: Entity) -> Self {
        Self {
            target,
            commands,
            _marker: PhantomData,
        }
    }

    /// Returns a reborrowed spawner command with a shorter lifetime.
    pub fn reborrow(&mut self) -> RelatedSpawnerCommands<'_, R> {
        RelatedSpawnerCommands {
            target: self.target,
            commands: self.commands.reborrow(),
            _marker: PhantomData,
        }
    }

    /// Spawns an entity with the given `bundle` and an `R` relationship targeting the `target`
    /// entity this spawner was initialized with.
    pub fn spawn(&mut self, bundle: impl Bundle) -> EntityCommands<'_> {
        self.commands.spawn((R::from_target(self.target), bundle))
    }

    /// Spawns an entity with an `R` relationship targeting the `target`
    /// entity this spawner was initialized with.
    pub fn spawn_empty(&mut self) -> EntityCommands<'_> {
        self.commands.spawn(R::from_target(self.target))
    }

    /// Returns the "target entity" used when spawning entities with an `R` [`Relationship`].
    pub fn target_entity(&self) -> Entity {
        self.target
    }

    /// Returns the underlying [`Commands`].
    pub fn commands(&mut self) -> Commands<'_, '_> {
        self.commands.reborrow()
    }

    /// Returns a mutable reference to the underlying [`Commands`].
    pub fn commands_mut(&mut self) -> &mut Commands<'w, 'w> {
        &mut self.commands
    }
}
