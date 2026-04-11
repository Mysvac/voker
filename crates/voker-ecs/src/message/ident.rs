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
/// struct Collision { /* .. */ }
///
/// let mut world = World::alloc();
/// world.register_message::<Collision>();
///
/// world.write_message(Collision { /* .. */ });
///
/// world.update_messages();
/// ```
///
/// [`Messages<T>`]: crate::message::Messages
/// [`World::register_message`]: crate::world::World::register_message
/// [`World::update_messages`]: crate::world::World::update_messages
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a message",
    label = "invalid message",
    note = "Consider annotating `{Self}` with `#[derive(Message)]`."
)]
pub trait Message: Send + Sync + 'static {}

// -----------------------------------------------------------------------------
// MessageId

/// A type used to represent a message index.
///
/// The internal value is allowed to wrap around, and users
/// should not store it for an excessively long time.
///
/// Although `usize` usually does not overflow wrap.
#[repr(transparent)]
pub struct MessageId<M: Message> {
    id: usize,
    _marker: PhantomData<M>,
}

impl<M: Message> MessageId<M> {
    #[inline(always)]
    pub(super) const fn new(id: usize) -> Self {
        MessageId {
            id,
            _marker: PhantomData,
        }
    }

    /// Creates a new `MessageId` from a usize.
    #[inline(always)]
    pub const fn without_provenance(id: usize) -> Self {
        Self {
            id,
            _marker: PhantomData,
        }
    }

    /// Returns the archetype index as a usize.
    #[inline(always)]
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
        write!(f, "message<{}>#{}", DebugName::type_name::<M>(), self.id)
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
        // Non-wrapping difference between two generations after
        // which a signed interpretation becomes negative.
        const DIFF_MAX: usize = usize::MAX >> 1;

        match self.id.wrapping_sub(other.id) {
            0 => Ordering::Equal,
            1..DIFF_MAX => Ordering::Greater,
            _ => Ordering::Less,
        }
    }
}

impl<M: Message> Hash for MessageId<M> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(self.id);
    }
}
