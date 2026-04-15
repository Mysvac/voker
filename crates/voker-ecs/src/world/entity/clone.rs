use crate::entity::Entity;
use crate::utils::DebugLocation;
use crate::world::EntityOwned;

impl EntityOwned<'_> {
    /// Despawns the current entity.
    ///
    /// If `self.location` is none, this function is no-op.
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn clone(&mut self, linked_clone: bool) -> Entity {
        self.clone_with_caller(linked_clone, DebugLocation::caller())
    }

    /// Despawns the current entity.
    ///
    /// If `self.location` is none, this function is no-op.
    pub(crate) fn clone_with_caller(
        &mut self,
        linked_clone: bool,
        caller: DebugLocation,
    ) -> Entity {
        self.assert_is_spawned_with_caller(caller);

        let mut cloner = unsafe { self.world.full_mut().entity_cloner() };

        let result = cloner.spawn_clone(self.entity, linked_clone);

        self.relocate();

        result
    }
}
