#![expect(clippy::module_inception, reason = "For better structure.")]

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::Any;
use core::fmt::Debug;

use voker_ptr::PtrMut;
use voker_utils::hash::SparseHashSet;

use crate::archetype::ObserverFlags;
use crate::component::ComponentId;
use crate::entity::Entity;
use crate::error::{ErrorContext, ErrorHandler, GameError};
use crate::event::{EventContext, EventId};
use crate::observer::ObserverId;
use crate::system::System;
use crate::utils::DebugName;
use crate::world::DeferredWorld;

// -----------------------------------------------------------------------------
// ObserverRunner

pub type ObserverRunner =
    unsafe fn(DeferredWorld, EventContext, ObserverId, event: PtrMut, trigger: PtrMut);

// -----------------------------------------------------------------------------
// Observer

pub(crate) trait NamedSystem: Any + Send + Sync + 'static {
    fn system_name(&self) -> DebugName;
}

impl<T: Any + System> NamedSystem for T {
    fn system_name(&self) -> DebugName {
        self.id().name()
    }
}

// -----------------------------------------------------------------------------
// Observer

pub struct Observer {
    pub(crate) event_id: EventId,
    pub(crate) last_trigger: u32,
    pub(crate) system: Box<dyn NamedSystem>,
    pub(crate) runner: ObserverRunner,
    pub(crate) error_handler: Option<ErrorHandler>,
    pub(crate) observed_components: Vec<ComponentId>,
    pub(crate) observed_entities: SparseHashSet<Entity>,
}

impl Debug for Observer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Observer").field(&self.system.system_name()).finish()
    }
}

impl Observer {
    pub fn system_name(&self) -> DebugName {
        self.system.system_name()
    }

    pub fn with_entity(mut self, entity: Entity) -> Self {
        self.observed_entities.insert(entity);
        self
    }

    pub fn with_entities<I: IntoIterator<Item = Entity>>(mut self, entities: I) -> Self {
        self.observed_entities.extend(entities);
        self
    }

    pub fn with_component(mut self, component: ComponentId) -> Self {
        self.observed_components.push(component);
        self.observed_components.sort();
        self.observed_components.dedup();
        self
    }

    pub fn with_components<I: IntoIterator<Item = ComponentId>>(mut self, components: I) -> Self {
        self.observed_components.extend(components);
        self.observed_components.sort();
        self.observed_components.dedup();
        self
    }

    pub fn with_error_handler(mut self, error_handler: fn(GameError, ErrorContext)) -> Self {
        self.error_handler = Some(error_handler);
        self
    }

    pub(crate) fn flags(&self) -> Option<ObserverFlags> {
        use crate::event::*;
        match self.event_id {
            ADD => Some(ObserverFlags::ADD),
            CLONE => Some(ObserverFlags::CLONE),
            INSERT => Some(ObserverFlags::INSERT),
            REMOVE => Some(ObserverFlags::REMOVE),
            DISCARD => Some(ObserverFlags::DISCARD),
            DESPAWN => Some(ObserverFlags::DESPAWN),
            _ => None,
        }
    }
}
