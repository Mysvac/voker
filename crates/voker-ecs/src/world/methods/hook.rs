use crate::component::{Component, ComponentHooks};
use crate::utils::DebugName;
use crate::world::World;

impl World {
    /// Returns mutable lifecycle hooks for component `C`.
    ///
    /// Hooks must be configured before `C` appears in any archetype so all
    /// entities using `C` observe a consistent hook set.
    ///
    /// # Panics
    ///
    /// Panics if `C` is already present in at least one archetype.
    #[track_caller]
    pub fn register_component_hook<C: Component>(&mut self) -> &mut ComponentHooks {
        let id = self.components.register::<C>();

        if self.archetypes.iter().any(|a| a.contains_component(id)) {
            core::hint::cold_path();
            panic! {
                "ComponentHook cannot be modified if the component \
                already exists in an archetype {} .",
                DebugName::type_name::<C>()
            }
        }
        unsafe { self.components.get_unchecked_mut(id).hooks_mut() }
    }
}
