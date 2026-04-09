use alloc::boxed::Box;
use voker_utils::hash::NoOpHashMap;

use crate::error::GameError;
use crate::prelude::Resource;
use crate::system::{IntoSystem, System, SystemId, SystemInput};
use crate::world::World;

// -----------------------------------------------------------------------------
// SystemComponent

/// Type-erased system object stored in world-level system resources.
type BoxedSystem<I, O> = Box<dyn System<Input = I, Output = O>>;

/// Per-signature cache for systems keyed by [`SystemId`].
///
/// The generic parameters (`I`, `O`) partition caches by system input/output
/// signature so ids do not collide across incompatible system types.
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
    /// Runs a previously prepared system, then places it back into the cache.
    #[inline(never)]
    fn run_system_internal<I, O>(
        &mut self,
        mut system: BoxedSystem<I, O>,
        input: I::Data<'_>,
    ) -> Result<O, GameError>
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

    /// Temporarily removes a cached system from storage.
    fn take_system<I, O>(&mut self, id: SystemId) -> Option<BoxedSystem<I, O>>
    where
        I: SystemInput + 'static,
        O: 'static,
    {
        let res = self.get_resource_mut::<SystemResource<I, O>>()?;
        res.into_inner().mapper.get_mut(&id)?.take()
    }

    /// Converts an [`IntoSystem`] into a boxed runtime system and initializes it.
    ///
    /// This does not register the system by itself. Registration happens when
    /// the system is inserted into the cache by run or register APIs.
    fn build_system<I, O, M>(
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

    /// Registers a system into the world cache if absent.
    pub fn register_system<I, O, M>(&mut self, system: impl IntoSystem<I, O, M> + 'static)
    where
        I: SystemInput + 'static,
        O: 'static,
    {
        let res = self.resource_mut_or_init::<SystemResource<I, O>>();
        let res = res.into_inner();
        let id = system.system_id();
        if matches!(res.mapper.get_mut(&id), Some(Some(_))) {
            return;
        }
        res.mapper.insert(id, Some(Box::new(IntoSystem::into_system(system))));
    }

    /// Removes a registered system from cache and returns ownership of it.
    ///
    /// Returns `None` when the typed cache or entry does not exist.
    pub fn unregister_system<I, O, M>(&mut self, system: impl IntoSystem<I, O, M> + 'static)
    where
        I: SystemInput + 'static,
        O: 'static,
    {
        if let Some(res) = self.get_resource_mut::<SystemResource<I, O>>() {
            res.into_inner().mapper.remove(&system.system_id());
        }
    }

    /// Runs a system with explicit input data.
    ///
    /// This function automatically registers the system if it is not already
    /// registered.
    ///
    /// The system is looked up by [`SystemId`]. If not already cached, it is
    /// built and initialized first. After execution, it remains cached.
    #[inline]
    pub fn run_system_with<I, O, M>(
        &mut self,
        system: impl IntoSystem<I, O, M> + 'static,
        input: I::Data<'_>,
    ) -> Result<O, GameError>
    where
        I: SystemInput + 'static,
        O: 'static,
    {
        let opt = self.take_system::<I, O>(system.system_id());
        let system = opt.unwrap_or_else(|| self.build_system::<I, O, M>(system));
        self.run_system_internal::<I, O>(system, input)
    }

    /// Runs a named system with unit input.
    ///
    /// This function automatically registers the system if it is not already
    /// registered.
    ///
    /// The system is looked up by [`SystemId`]. If not already cached, it is
    /// built and initialized first. After execution, it remains cached.
    #[inline]
    pub fn run_system<O: 'static, M>(
        &mut self,
        system: impl IntoSystem<(), O, M> + 'static,
    ) -> Result<O, GameError> {
        let opt = self.take_system::<(), O>(system.system_id());
        let system = opt.unwrap_or_else(|| self.build_system::<(), O, M>(system));
        self.run_system_internal::<(), O>(system, ())
    }

    /// Runs a registered system by id and returns the input back on cache miss.
    ///
    /// If the system is not registered, this function returns the input value
    /// unchanged.
    ///
    /// This means you should call [`World::register_system`] before calling
    /// this function.
    pub fn run_system_cached<'i, I, O>(
        &mut self,
        system_id: SystemId,
        input: I::Data<'i>,
    ) -> Result<Result<O, GameError>, I::Data<'i>>
    where
        I: SystemInput + 'static,
        O: 'static,
    {
        match self.take_system::<I, O>(system_id) {
            Some(system) => Ok(self.run_system_internal::<I, O>(system, input)),
            None => Err(input),
        }
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
