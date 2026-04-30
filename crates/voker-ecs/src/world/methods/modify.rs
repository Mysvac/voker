use crate::borrow::UntypedMut;
use crate::component::Component;
use crate::entity::{Entity, FetchError};
use crate::event::{DISCARD, Discard, EntityComponentsTrigger, INSERT, Insert};
use crate::prelude::ComponentId;
use crate::utils::DebugLocation;
use crate::world::World;

impl World {
    /// Mutates component `C` on `entity` and executes replacement hooks.
    ///
    /// Returns:
    /// - `Ok(Some(result))` when the entity exists and has component `C`.
    /// - `Ok(None)` when the entity exists but does not contain `C` (or `C`
    ///   was never registered).
    /// - `Err(FetchError)` when `entity` is not currently spawned.
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn modify_component<C: Component, R>(
        &mut self,
        entity: Entity,
        f: impl FnOnce(&mut C) -> R,
    ) -> Result<Option<R>, FetchError> {
        let caller = DebugLocation::caller();
        self.modify_component_with_caller(entity, caller, f)
    }

    #[inline]
    pub(crate) fn modify_component_with_caller<T: Component, R>(
        &mut self,
        entity: Entity,
        caller: DebugLocation,
        f: impl FnOnce(&mut T) -> R,
    ) -> Result<Option<R>, FetchError> {
        // If the component is not registered, then it doesn't exist on this entity, so no action required.
        let Some(component_id) = self.get_component_id::<T>() else {
            return Ok(None);
        };

        modify_component_internal(self, entity, component_id, caller, move |component| {
            f(unsafe { component.with_type::<T>().into_inner() })
        })
    }
}

pub(crate) fn modify_component_internal<R>(
    world: &mut World,
    entity: Entity,
    component_id: ComponentId,
    caller: DebugLocation,
    f: impl for<'a> FnOnce(UntypedMut<'a>) -> R,
) -> Result<Option<R>, FetchError> {
    let entity_mut = world.get_entity_mut(entity)?;
    let arche = entity_mut.archetype();

    if !arche.contains_component(component_id) {
        return Ok(None);
    }

    let has_on_discard_hooks = !arche.on_discard_hooks().is_empty();
    let has_on_insert_hooks = !arche.on_insert_hooks().is_empty();
    let has_on_discard_observer = arche.has_on_discard_observer();
    let has_on_insert_observer = arche.has_on_insert_observer();

    let arche = &raw const *arche;

    unsafe {
        if has_on_discard_hooks {
            let iter = core::iter::once(component_id);
            world.deferred().trigger_on_discard(entity, iter, caller);
        }
        if has_on_discard_observer {
            let mut event = Discard { entity };
            let mut trigger = EntityComponentsTrigger {
                components: &[component_id],
                old_archetype: Some(&*arche),
                new_archetype: Some(&*arche),
            };
            world
                .deferred()
                .trigger_raw(DISCARD, &mut event, &mut trigger, caller);
        }
    }

    let mut entity_mut = world.get_entity_mut(entity).expect("entity access confirmed above");

    // SAFETY: we will run the required hooks to simulate removal/replacement.
    let mut component = entity_mut
        .get_mut_by_id(component_id)
        .expect("component access confirmed above");

    let result = f(component.reborrow());

    unsafe {
        if has_on_insert_hooks {
            let iter = core::iter::once(component_id);
            world.deferred().trigger_on_insert(entity, iter, caller);
        }
        if has_on_insert_observer {
            let mut event = Insert { entity };
            let mut trigger = EntityComponentsTrigger {
                components: &[component_id],
                old_archetype: Some(&*arche),
                new_archetype: Some(&*arche),
            };
            world.deferred().trigger_raw(INSERT, &mut event, &mut trigger, caller);
        }
    }

    Ok(Some(result))
}
