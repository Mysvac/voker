//! A trait for components that let you traverse the ECS.

use crate::entity::Entity;
use crate::relationship::Relationship;
use crate::world::EntityRef;

/// A component that can point to another entity, and which
/// can be used to define a path through the ECS.
///
/// Traversals are used to [specify the direction] of [event propagation]
/// in [`EntityEvent`] [observers]. The default query is `()`.
///
/// Infinite loops are possible, and are not checked for. While looping can be
/// desirable in some contexts (for example, an observer that triggers itself
/// multiple times before stopping), following an infinite is responsible for
/// documenting possible looping behavior, and consumers of those implementations
/// are responsible for avoiding infinite loops in their code.
///
/// Traversals may be parameterized with additional data. For example, in observer
/// event propagation, the parameter `D` is the event type given in `On<E>`.
/// This allows traversal to differ depending on event data.
///
/// [specify the direction]: crate::event::PropagateEntityTrigger
/// [event propagation]: crate::observer::On::set_propagate
/// [observers]: crate::observer::Observer
/// [`EntityEvent`]: crate::event::EntityEvent
pub trait Traversal<E: ?Sized> {
    fn traverse(source: EntityRef, data: &E) -> Option<Entity>;
}

impl<E: ?Sized> Traversal<E> for () {
    fn traverse(_source: EntityRef, _data: &E) -> Option<Entity> {
        None
    }
}

impl<R: Relationship, E> Traversal<E> for &R {
    fn traverse(source: EntityRef, _data: &E) -> Option<Entity> {
        Some(source.get::<R>()?.related_target())
    }
}
