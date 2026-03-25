use core::cmp::Ordering;
use core::fmt::{Debug, Display};
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;

use crate::utils::DebugName;

// -----------------------------------------------------------------------------
// Message

/// Marker trait for ECS message payload types.
///
/// Message values are stored inside [`Messages<T>`]. To participate in automatic
/// lifecycle rotation, register the type with  [`World::register_message`] and run
/// [`World::update_messages`] each update.
///
/// # Example
///
/// ```rust
/// use voker_ecs::prelude::*;
///
/// #[derive(Message)]
/// struct Collision;
///
/// let mut world = World::alloc();
/// world.register_message::<Collision>();
///
/// world
///     .resource_mut_or_init::<Messages<Collision>>()
///     .write(Collision);
///
/// world.update_messages();
/// ```
///
/// [`Messages<T>`]: crate::message::Messages
/// [`World::register_message`]: crate::world::World::register_message
/// [`World::update_messages`]: crate::world::World::update_messages
#[diagnostic::on_unimplemented(note = "consider annotating `{Self}` with `#[derive(Message)]`")]
pub trait Message: Send + Sync + 'static {}

// -----------------------------------------------------------------------------
// MessageId

pub struct MessageId<M: Message> {
    id: usize,
    _marker: PhantomData<M>,
}

impl<M: Message> MessageId<M> {
    #[inline]
    pub(super) const fn new(id: usize) -> Self {
        MessageId {
            id,
            _marker: PhantomData,
        }
    }

    pub const fn index(self) -> usize {
        self.id
    }
}

impl<M: Message> Copy for MessageId<M> {}

impl<M: Message> Clone for MessageId<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M: Message> Display for MessageId<M> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        <Self as Debug>::fmt(self, f)
    }
}

impl<M: Message> Debug for MessageId<M> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "message<{}>#{}", DebugName::type_name::<M>(), self.id,)
    }
}

impl<M: Message> PartialEq for MessageId<M> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<M: Message> Eq for MessageId<M> {}

impl<M: Message> PartialOrd for MessageId<M> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<M: Message> Ord for MessageId<M> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl<M: Message> Hash for MessageId<M> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(self.id);
    }
}
