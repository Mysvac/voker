use crate::bundle::Bundle;
use crate::entity::Entity;
use crate::{utils::DebugLocation, world::EntityOwned};

use super::{LinkHookMode, LinkSource};

impl<'w> EntityOwned<'w> {
    /// Spawns a entity related to this entity (with the `R` relationship) by taking a bundle
    pub fn with_linked<R: LinkSource>(&mut self, bundle: impl Bundle) -> &mut Self {
        let target = self.entity();

        self.world_scope(|world| {
            world.spawn((bundle, R::from_target(target)));
        });

        self
    }

    // pub fn add_related<R: LinkSource>(&mut self, related: &[Entity]) -> &mut Self {
    //     let id = self.entity();
    //     self.world_scope(|world| {
    //         for related in related {
    //             world
    //                 .entity_mut(*related)
    //                 .modify_or_insert_link_with_hook_mode::<R>(
    //                     id,
    //                     LinkHookMode::Run,
    //                 );
    //         }
    //     });
    //     self
    // }

    #[expect(unused, reason = "todo")]
    fn modify_or_insert_link_with_hook_mode<L: LinkSource>(
        &mut self,
        entity: Entity,
        link_hook_mode: LinkHookMode,
    ) {
        self.assert_is_spawned_with_caller(DebugLocation::caller());

        // if size_of::<L>() > size_of::<Entity>() {
        //     let this = self.entity();

        //     let modified = self.world_scope(|world| {
        //         let modified = world.deferred()
        //             .modify_component_with_link_hook_mode::<R, _>(
        //                 this,
        //                 relationship_hook_mode,
        //                 |r| r.set_risky(entity),
        //             )
        //             .expect("entity access must be valid")
        //             .is_some();

        //         world.flush();

        //         modified
        //     });

        //     if modified {
        //         return;
        //     }
        // }

        // self.insert_with_link_hook_mode(R::from(entity), link_hook_mode);
    }
}
