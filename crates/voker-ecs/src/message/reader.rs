use crate::borrow::Res;
use crate::message::{Message, MessageCursor, Messages};
use crate::message::{MessageIterator, MessageWithIdIterator};
use crate::system::{Local, ReadOnlySystemParam, SystemParam};

/// Read-only system parameter for consuming unread messages of type `M`.
///
/// Each system instance keeps its own local [`MessageCursor`], so independent
/// systems can read the same messages without interfering with each other.
///
/// # Example
///
/// ```rust
/// use voker_ecs::prelude::*;
///
/// #[derive(Message)]
/// struct Collision {
///     lhs: u32,
///     rhs: u32,
/// }
///
/// fn handle_collisions(mut reader: MessageReader<Collision>) {
///     for collision in reader.read() {
///         let _ = (collision.lhs, collision.rhs);
///     }
/// }
/// ```
pub struct MessageReader<'w, 's, M: Message> {
    cursor: Local<'s, MessageCursor<M>>,
    messages: Res<'w, Messages<M>>,
}

impl<'w, 's, M: Message> MessageReader<'w, 's, M> {
    /// Returns an iterator over unread messages for this reader cursor.
    pub fn read(&mut self) -> MessageIterator<'_, M> {
        self.cursor.read(&self.messages)
    }

    /// Returns unread messages together with their [`crate::message::MessageId`].
    pub fn read_with_id(&mut self) -> MessageWithIdIterator<'_, M> {
        self.cursor.read_with_id(&self.messages)
    }

    /// Returns the number of unread messages for this reader cursor.
    pub fn len(&self) -> usize {
        self.cursor.len(&self.messages)
    }

    /// Returns `true` if there are no unread messages for this reader cursor.
    pub fn is_empty(&self) -> bool {
        self.cursor.is_empty(&self.messages)
    }

    /// Marks all currently readable messages as seen for this cursor.
    pub fn clear(&mut self) {
        self.cursor.clear(&self.messages);
    }
}

// SAFETY: Delegates access and parameter building to `(Local<MessageCursor<M>>, Res<Messages<M>>)`.
unsafe impl<M: Message> ReadOnlySystemParam for MessageReader<'_, '_, M> {}

type InternalParam<M> = (Local<'static, MessageCursor<M>>, Res<'static, Messages<M>>);

// SAFETY: Delegates state, access declaration, and value fetching to tuple param impl.
unsafe impl<M: Message> SystemParam for MessageReader<'_, '_, M> {
    type State = <InternalParam<M> as SystemParam>::State;

    type Item<'world, 'state> = MessageReader<'world, 'state, M>;

    const NON_SEND: bool = false;
    const EXCLUSIVE: bool = false;

    fn init_state(world: &mut crate::world::World) -> Self::State {
        <InternalParam<M> as SystemParam>::init_state(world)
    }

    fn mark_access(table: &mut crate::system::AccessTable, state: &Self::State) -> bool {
        <InternalParam<M> as SystemParam>::mark_access(table, state)
    }

    unsafe fn build_param<'w, 's>(
        world: crate::world::UnsafeWorld<'w>,
        state: &'s mut Self::State,
        last_run: crate::tick::Tick,
        this_run: crate::tick::Tick,
    ) -> Result<Self::Item<'w, 's>, crate::error::EcsError> {
        // SAFETY: same world/state/tick contract as delegated tuple parameter.
        let (cursor, messages) = unsafe {
            <InternalParam<M> as SystemParam>::build_param(world, state, last_run, this_run)?
        };

        Ok(MessageReader { cursor, messages })
    }
}
