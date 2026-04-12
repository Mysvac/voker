use core::cmp::Ordering;
use core::fmt::{Debug, Display};
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;

use crate::utils::DebugName;

// -----------------------------------------------------------------------------
// Message

/// Marker trait for ECS message payload types.
///
/// A `Message` type is a short-lived payload sent between systems through
/// [`Messages<T>`]. The trait has no methods: it only encodes bounds required
/// by message storage and cross-system usage.
///
/// For user code, the recommended path is `#[derive(Message)]`.
///
/// To participate in automatic lifecycle rotation, register the type with
/// [`World::register_message`] and run [`World::update_messages`] each update.
///
/// # Using Messages In World
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
/// # Using Messages In Systems
///
/// `Message` is consumed through system parameters in three roles:
/// - [`MessageWriter<T>`]: append new messages.
/// - [`MessageReader<T>`]: read unread messages immutably.
/// - [`MessageMutator<T>`]: read unread messages mutably.
///
/// `MessageReader` and `MessageMutator` each keep an independent local cursor,
/// so one system reading messages does not consume them for another system.
///
/// ```rust
/// use voker_ecs::prelude::*;
///
/// #[derive(Message)]
/// struct Damage {
///     amount: u32,
/// }
///
/// fn emit(mut writer: MessageWriter<Damage>) {
///     writer.write(Damage { amount: 120 });
/// }
///
/// fn clamp(mut mutator: MessageMutator<Damage>) {
///     for msg in mutator.read() {
///         msg.amount = msg.amount.min(100);
///     }
/// }
///
/// fn log(mut reader: MessageReader<Damage>) {
///     for msg in reader.read() {
///         let _ = msg.amount;
///     }
/// }
/// ```
///
/// [`Messages<T>`]: crate::message::Messages
/// [`MessageWriter<T>`]: crate::message::MessageWriter
/// [`MessageReader<T>`]: crate::message::MessageReader
/// [`MessageMutator<T>`]: crate::message::MessageMutator
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

/// Identifier for one message in a `Messages<M>` stream.
///
/// `MessageId` is backed by a wrapping `usize` counter. It is stable for
/// correlation within the stream (for example, tracking ids returned by
/// `write_batch`), but callers should avoid treating it as a globally monotonic
/// timestamp across very long runtimes.
///
/// Ordering is wrap-aware and designed for stream-local comparisons.
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

    /// Returns the raw message index as `usize`.
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
