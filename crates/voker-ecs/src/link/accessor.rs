use alloc::boxed::Box;

use voker_ptr::Ptr;

use crate::component::{ComponentId, Components};
use crate::entity::Entity;
use crate::link::{LinkSource, LinkTarget};

#[derive(Clone, Copy, Debug)]
pub enum LinkRegistrar {
    LinkSource {
        entity_field_offset: usize,
        linked_spawn: bool,
        allow_self_linking: bool,
        link_target_register_fn: fn(&mut Components) -> ComponentId,
    },
    LinkTarget {
        iter: for<'a> unsafe fn(Ptr<'a>) -> Box<dyn Iterator<Item = Entity> + 'a>,
        linked_spawn: bool,
        allow_self_linking: bool,
        link_source_register_fn: fn(&mut Components) -> ComponentId,
    },
}

#[derive(Clone, Copy, Debug)]
pub enum LinkAccessor {
    LinkSource {
        entity_field_offset: usize,
        linked_spawn: bool,
        allow_self_linking: bool,
        link_target_id: ComponentId,
    },
    LinkTarget {
        iter: for<'a> unsafe fn(Ptr<'a>) -> Box<dyn Iterator<Item = Entity> + 'a>,
        linked_spawn: bool,
        allow_self_linking: bool,
        link_source_id: ComponentId,
    },
}

impl LinkRegistrar {
    pub const fn link_source<C: LinkSource>(entity_field_offset: usize) -> Self {
        Self::LinkSource {
            entity_field_offset,
            linked_spawn: C::LINKED_SPAWN,
            allow_self_linking: C::ALLOW_SELF_LINKING,
            link_target_register_fn: Components::register::<C::Target>,
        }
    }

    pub const fn link_target<C: LinkTarget>() -> Self {
        Self::LinkTarget {
            iter: |ptr| unsafe { Box::new(LinkTarget::iter(ptr.deref::<C>())) },
            linked_spawn: <C::Source as LinkSource>::LINKED_SPAWN,
            allow_self_linking: <C::Source as LinkSource>::ALLOW_SELF_LINKING,
            link_source_register_fn: Components::register::<C::Source>,
        }
    }

    pub fn register(self, components: &mut Components) -> LinkAccessor {
        match self {
            LinkRegistrar::LinkSource {
                entity_field_offset,
                linked_spawn,
                allow_self_linking,
                link_target_register_fn,
            } => {
                let link_target_id = link_target_register_fn(components);
                LinkAccessor::LinkSource {
                    entity_field_offset,
                    linked_spawn,
                    allow_self_linking,
                    link_target_id,
                }
            }
            LinkRegistrar::LinkTarget {
                iter,
                linked_spawn,
                allow_self_linking,
                link_source_register_fn,
            } => {
                let link_source_id = link_source_register_fn(components);
                LinkAccessor::LinkTarget {
                    iter,
                    linked_spawn,
                    allow_self_linking,
                    link_source_id,
                }
            }
        }
    }
}
