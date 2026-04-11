use alloc::boxed::Box;

use voker_ptr::Ptr;

use crate::component::{ComponentId, Components};
use crate::entity::Entity;
use crate::relationship::{Relationship, RelationshipTarget};

#[derive(Clone, Copy, Debug)]
pub enum RelationshipRegistrar {
    Relationship {
        target_offset: usize,
        linked_lifecycle: bool,
        allow_self_referential: bool,
        relationship_target_register_fn: fn(&mut Components) -> ComponentId,
    },
    RelationshipTarget {
        sources_iter: for<'a> unsafe fn(Ptr<'a>) -> Box<dyn Iterator<Item = Entity> + 'a>,
        linked_lifecycle: bool,
        allow_self_referential: bool,
        relationship_register_fn: fn(&mut Components) -> ComponentId,
    },
}

#[derive(Clone, Copy, Debug)]
pub enum RelationshipAccessor {
    Relationship {
        target_offset: usize,
        linked_lifecycle: bool,
        allow_self_referential: bool,
        relationship_target_id: ComponentId,
    },
    RelationshipTarget {
        sources_iter: for<'a> unsafe fn(Ptr<'a>) -> Box<dyn Iterator<Item = Entity> + 'a>,
        linked_lifecycle: bool,
        allow_self_referential: bool,
        relationship_id: ComponentId,
    },
}

impl RelationshipRegistrar {
    pub const fn relationship<C: Relationship>() -> Self {
        Self::Relationship {
            target_offset: C::TARGET_FIELD_OFFSET,
            linked_lifecycle: <C::RelationshipTarget as RelationshipTarget>::LINKED_LIFECYCLE,
            allow_self_referential: C::ALLOW_SELF_REFERENTIAL,
            relationship_target_register_fn: Components::register::<C::RelationshipTarget>,
        }
    }

    pub const fn relationship_target<C: RelationshipTarget>() -> Self {
        Self::RelationshipTarget {
            sources_iter: |ptr| unsafe { Box::new(RelationshipTarget::iter(ptr.deref::<C>())) },
            linked_lifecycle: C::LINKED_LIFECYCLE,
            allow_self_referential: <C::Relationship as Relationship>::ALLOW_SELF_REFERENTIAL,
            relationship_register_fn: Components::register::<C::Relationship>,
        }
    }

    pub fn register(self, components: &mut Components) -> RelationshipAccessor {
        match self {
            RelationshipRegistrar::Relationship {
                target_offset,
                linked_lifecycle,
                allow_self_referential,
                relationship_target_register_fn,
            } => {
                let relationship_target_id = relationship_target_register_fn(components);
                RelationshipAccessor::Relationship {
                    target_offset,
                    linked_lifecycle,
                    allow_self_referential,
                    relationship_target_id,
                }
            }
            RelationshipRegistrar::RelationshipTarget {
                sources_iter,
                linked_lifecycle,
                allow_self_referential,
                relationship_register_fn,
            } => {
                let relationship_id = relationship_register_fn(components);
                RelationshipAccessor::RelationshipTarget {
                    sources_iter,
                    linked_lifecycle,
                    allow_self_referential,
                    relationship_id,
                }
            }
        }
    }
}
