use alloc::boxed::Box;

use crate::component::Component;
use crate::entity::Entity;
use crate::error::ErrorContext;
use crate::system::{IntoSystem, System, SystemInput};
use crate::world::World;

// -----------------------------------------------------------------------------
// SystemComponent

/// Erased system object stored in world-level registry entities.
type BoxedSystem<I, O> = Box<dyn System<Input = I, Output = O>>;

/// Component payload used to store one registered boxed system on an entity.
///
/// Wrapped in `Option` so a system can be temporarily taken out for execution
/// and then placed back into storage.
struct SystemComponent<I: SystemInput, O> {
    system: Option<BoxedSystem<I, O>>,
}

impl<I: SystemInput + 'static, O: 'static> Component for SystemComponent<I, O> {}

// -----------------------------------------------------------------------------
// SystemRegistry, SystemComponent

impl World {
    #[cold]
    #[inline(never)]
    fn register_boxed_system<I, O>(&mut self, mut system: BoxedSystem<I, O>) -> Entity
    where
        I: SystemInput + 'static,
        O: 'static,
    {
        // Ensure param/access metadata is prepared before first run.
        let _ = system.initialize(self);
        let id = system.id();

        let data = SystemComponent {
            system: Some(system),
        };
        let entity = self.spawn(data).entity();
        self.system_registry.insert(id, entity);

        entity
    }

    fn run_system_internal<I, O>(&mut self, entity: Entity, input: I::Data<'_>) -> Option<O>
    where
        I: SystemInput + 'static,
        O: 'static,
    {
        let mut entity_mut = self.entity_mut(entity);
        let mut data = entity_mut.get_mut::<SystemComponent<I, O>>()?;
        let mut system = data.system.take()?;

        // SAFETY: the registered system is executed against the current world
        // following the same contracts as schedule-driven execution.
        let ret = unsafe { system.run(input, self.unsafe_world()) };

        // Put the system back to preserve registration for future runs.
        let mut entity_mut = self.entity_mut(entity);
        let mut data = entity_mut.get_mut::<SystemComponent<I, O>>().unwrap();

        match ret {
            Ok(ret) => {
                data.system = Some(system);
                Some(ret)
            }
            Err(e) => {
                voker_utils::cold_path();
                let id = system.id();
                let last_run = system.get_last_run();
                data.system = Some(system);
                let hander = self.default_error_handler();
                let ctx = ErrorContext::System { id, last_run };
                (hander.0)(e, ctx);
                None
            }
        }
    }

    #[inline]
    pub fn run_system_with<I, O, M>(
        &mut self,
        system: impl IntoSystem<I, O, M> + 'static,
        input: I::Data<'_>,
    ) -> Option<O>
    where
        I: SystemInput + 'static,
        O: 'static,
    {
        let id = system.system_id();
        let entity = self.system_registry.get(id).unwrap_or_else(|| {
            self.register_boxed_system(Box::new(IntoSystem::into_system(system)))
        });

        self.run_system_internal::<I, O>(entity, input)
    }

    /// Runs a named system with unit input.
    #[inline]
    pub fn run_system<O: 'static, M>(
        &mut self,
        system: impl IntoSystem<(), O, M> + 'static,
    ) -> Option<O> {
        self.run_system_with::<(), O, M>(system, ())
    }
}

#[cfg(test)]
mod tests {
    use crate::{name::Name, query::Query, world::World};

    #[test]
    fn temp() {
        let mut world = World::alloc();

        world.run_system(spawn);
        world.run_system(display);

        fn spawn(world: &mut World) {
            for id in 0..100 {
                world.spawn(Name::new(alloc::format!("id_{id}")));
            }
        }

        fn display(query: Query<&Name>) {
            for name in query {
                std::eprintln!("{name}");
            }
        }
    }
}
