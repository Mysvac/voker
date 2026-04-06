use alloc::boxed::Box;
use voker_utils::hash::NoOpHashMap;

use crate::error::EcsError;
use crate::prelude::Resource;
use crate::system::{IntoSystem, System, SystemId, SystemInput};
use crate::world::World;

// -----------------------------------------------------------------------------
// SystemComponent

/// Erased system object stored in world-level registry entities.
type BoxedSystem<I, O> = Box<dyn System<Input = I, Output = O>>;

struct SystemResource<I: SystemInput, O> {
    mapper: NoOpHashMap<SystemId, Option<BoxedSystem<I, O>>>,
}

impl<I: SystemInput + 'static, O: 'static> Default for SystemResource<I, O> {
    fn default() -> Self {
        Self {
            mapper: NoOpHashMap::new(),
        }
    }
}

impl<I: SystemInput + 'static, O: 'static> Resource for SystemResource<I, O> {}

// -----------------------------------------------------------------------------
// SystemRegistry, SystemComponent

impl World {
    #[inline(never)]
    fn run_system_internal<I, O>(
        &mut self,
        mut system: BoxedSystem<I, O>,
        input: I::Data<'_>,
    ) -> Result<O, EcsError>
    where
        I: SystemInput + 'static,
        O: 'static,
    {
        // SAFETY: the registered system is executed against the current world
        // following the same contracts as schedule-driven execution.
        let ret = unsafe { system.run(input, self.unsafe_world()) };
        let id = system.id();

        // Put the system back to preserve registration for future runs.
        let mut res = self.resource_mut_or_init::<SystemResource<I, O>>();
        let old = res.mapper.insert(id, Some(system));

        if matches!(old, Some(Some(_))) {
            log::warn!("The same new System `{id}` was inserted during the execution.");
        }

        ret
    }

    fn try_fetch_system<I, O>(&mut self, id: SystemId) -> Option<BoxedSystem<I, O>>
    where
        I: SystemInput + 'static,
        O: 'static,
    {
        let res = self.get_resource_mut::<SystemResource<I, O>>()?;
        res.into_inner().mapper.get_mut(&id)?.take()
    }

    pub fn build_system<I, O, M>(
        &mut self,
        system: impl IntoSystem<I, O, M> + 'static,
    ) -> BoxedSystem<I, O>
    where
        I: SystemInput + 'static,
        O: 'static,
    {
        let mut system = Box::new(IntoSystem::into_system(system));
        let _ = system.initialize(self);
        system
    }

    #[inline]
    pub fn run_system_with<I, O, M>(
        &mut self,
        system: impl IntoSystem<I, O, M> + 'static,
        input: I::Data<'_>,
    ) -> Result<O, EcsError>
    where
        I: SystemInput + 'static,
        O: 'static,
    {
        let opt = self.try_fetch_system::<I, O>(system.system_id());
        let system = opt.unwrap_or_else(|| self.build_system::<I, O, M>(system));
        self.run_system_internal::<I, O>(system, input)
    }

    /// Runs a named system with unit input.
    #[inline]
    pub fn run_system<O: 'static, M>(
        &mut self,
        system: impl IntoSystem<(), O, M> + 'static,
    ) -> Result<O, EcsError> {
        let opt = self.try_fetch_system::<(), O>(system.system_id());
        let system = opt.unwrap_or_else(|| self.build_system::<(), O, M>(system));
        self.run_system_internal::<(), O>(system, ())
    }
}

#[cfg(test)]
mod tests {
    use crate::{name::Name, query::Query, world::World};

    #[test]
    fn temp() {
        let mut world = World::alloc();

        world.run_system(spawn).unwrap();
        world.run_system(display).unwrap();

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
