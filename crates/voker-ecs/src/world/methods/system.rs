use alloc::boxed::Box;

use voker_utils::hash::NoOpHashMap;

use crate::component::Component;
use crate::entity::Entity;
use crate::error::ErrorContext;
use crate::prelude::Resource;
use crate::system::{IntoSystem, System, SystemInput, SystemName};
use crate::world::World;

// -----------------------------------------------------------------------------
// SystemRegistry, RegisteredSystem

/// Erased system object stored in world-level registry entities.
type BoxedSystem<I, O> = Box<dyn System<Input = I, Output = O>>;

/// Maps a [`SystemName`] to the entity that stores the registered system instance.
#[derive(Default)]
struct SystemRegistry {
    mapper: NoOpHashMap<SystemName, Entity>,
}

/// Component payload used to store one registered boxed system on an entity.
///
/// Wrapped in `Option` so a system can be temporarily taken out for execution
/// and then placed back into storage.
struct RegisteredSystem<I: SystemInput, O> {
    system: Option<BoxedSystem<I, O>>,
}

impl Resource for SystemRegistry {}

impl<I: SystemInput + 'static, O: 'static> Component for RegisteredSystem<I, O> {}

impl SystemRegistry {
    /// Inserts or replaces the entity binding of a system name.
    pub fn insert(&mut self, name: SystemName, entity: Entity) {
        self.mapper.insert(name, entity);
    }

    /// Removes a system binding and returns its entity if it exists.
    pub fn remove(&mut self, name: SystemName) -> Option<Entity> {
        self.mapper.remove(&name)
    }

    /// Returns the entity currently bound to a system name.
    pub fn get(&self, name: SystemName) -> Option<Entity> {
        self.mapper.get(&name).copied()
    }
}

// -----------------------------------------------------------------------------
// SystemRegistry, RegisteredSystem

impl World {
    /// Registers a system into world storage and returns its [`SystemName`].
    ///
    /// If another system with the same name already exists, it will be replaced.
    pub fn register_system<I, O, M>(
        &mut self,
        system: impl IntoSystem<I, O, M> + 'static,
    ) -> SystemName
    where
        I: SystemInput + 'static,
        O: 'static,
    {
        self.register_boxed_system(Box::new(IntoSystem::into_system(system)))
    }

    /// Separate to reduce compilation workload.
    ///
    /// Users can directly use `register_system`, it can also be used for `BoxedSystem`.
    #[inline(never)]
    fn register_boxed_system<I, O>(&mut self, mut system: BoxedSystem<I, O>) -> SystemName
    where
        I: SystemInput + 'static,
        O: 'static,
    {
        // Ensure param/access metadata is prepared before first run.
        let _ = system.initialize(self);

        let unsafe_world = self.unsafe_world();
        let world = unsafe { unsafe_world.data_mut() };
        let world_2 = unsafe { unsafe_world.full_mut() };

        let mut registry = world.resource_mut_or_init::<SystemRegistry>();
        let name = system.name();
        if let Some(old_entity) = registry.remove(name) {
            // Keep one active registration per system name.
            world_2.despawn(old_entity).unwrap();
        }

        let data = RegisteredSystem {
            system: Some(system),
        };
        let entity = world_2.spawn(data).entity();
        registry.insert(name, entity);

        name
    }

    /// Unregisters a named system and returns the boxed system instance.
    ///
    /// Returns `None` if the name is not registered or the expected typed payload
    /// does not match.
    pub fn unregister_system<I, O>(&mut self, name: SystemName) -> Option<BoxedSystem<I, O>>
    where
        I: SystemInput + 'static,
        O: 'static,
    {
        let registry = self.resource_mut::<SystemRegistry>()?;
        let entity = registry.get(name)?;
        let mut entity = self.entity_owned(entity);
        let mut registed = entity.get_mut::<RegisteredSystem<I, O>>()?;
        let system = registed.system.take();
        entity.despawn().unwrap();
        self.resource_mut::<SystemRegistry>().unwrap().remove(name);
        system
    }

    /// Runs a registered system with explicit input.
    ///
    /// The system is temporarily taken from storage, executed, then stored back.
    /// Any runtime error is sent to the world's default error handler.
    pub fn run_system_with<I, O>(&mut self, name: SystemName, input: I::Data<'_>) -> Option<O>
    where
        I: SystemInput + 'static,
        O: 'static,
    {
        let registry = self.resource_mut::<SystemRegistry>()?;
        let entity = registry.get(name)?;

        let mut entity_mut = self.entity_mut(entity);
        let mut data = entity_mut.get_mut::<RegisteredSystem<I, O>>()?;
        let mut system = data.system.take()?;

        let name = system.name();
        // SAFETY: the registered system is executed against the current world
        // following the same contracts as schedule-driven execution.
        let ret = unsafe { system.run(input, self.unsafe_world()) };

        // Put the system back to preserve registration for future runs.
        let mut entity_mut = self.entity_mut(entity);
        let mut data = entity_mut.get_mut::<RegisteredSystem<I, O>>().unwrap();
        data.system = Some(system);

        match ret {
            Ok(ret) => Some(ret),
            Err(e) => {
                voker_utils::cold_path();
                let hander = self.default_error_handler();
                let ctx = ErrorContext::System {
                    name,
                    last_run: self.last_run(),
                };
                (hander.0)(e, ctx);
                None
            }
        }
    }

    /// Runs a named system with unit input.
    pub fn run_system<O: 'static>(&mut self, name: SystemName) -> Option<O> {
        self.run_system_with::<(), O>(name, ())
    }
}
