use crate::component::{Component, ComponentHooks};
use crate::world::World;

impl World {
    pub fn register_component_hook<C: Component>(&mut self) -> &mut ComponentHooks {
        let id = self.components.register::<C>();

        if self.archetypes.iter().any(|a| a.contains_component(id)) {
            voker_utils::cold_path();
            panic! {
                "ComponentHook cannot be modified if the component\
                already exists in an archetype {} .",
                core::any::type_name::<C>()
            }
        }
        unsafe { self.components.get_unchecked_mut(id).hooks_mut() }
    }
}
