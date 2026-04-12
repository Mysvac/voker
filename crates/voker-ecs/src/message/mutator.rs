use crate::borrow::ResMut;
use crate::message::{Message, MessageCursor, Messages};
use crate::message::{MessageMutIterator, MessageMutWithIdIterator};
use crate::system::{AccessTable, Local, SystemParam, SystemParamError};

/// Mutable reader parameter for consuming and editing unread messages of type `M`.
///
/// Like [`crate::message::MessageReader`], each system instance maintains its
/// own local cursor.
///
/// Reading mutably still follows unread semantics: this parameter only yields
/// messages not yet observed by this system's cursor.
///
/// # Example
///
/// ```rust
/// use voker_ecs::prelude::*;
///
/// #[derive(Message)]
/// struct Damage {
///     amount: u32,
/// }
///
/// fn clamp_damage(mut mutator: MessageMutator<Damage>) {
///     for damage in mutator.read() {
///         damage.amount = damage.amount.min(100);
///     }
/// }
/// ```
pub struct MessageMutator<'w, 's, M: Message> {
    cursor: Local<'s, MessageCursor<M>>,
    messages: ResMut<'w, Messages<M>>,
}

impl<'w, 's, M: Message> MessageMutator<'w, 's, M> {
    /// Returns a mutable iterator over unread messages for this cursor.
    ///
    /// Iteration advances the cursor.
    pub fn read(&mut self) -> MessageMutIterator<'_, M> {
        self.cursor.read_mut(&mut self.messages)
    }

    /// Returns mutable unread messages together with their ids.
    pub fn read_with_id(&mut self) -> MessageMutWithIdIterator<'_, M> {
        self.cursor.read_mut_with_id(&mut self.messages)
    }

    /// Returns the number of unread messages for this cursor.
    pub fn len(&self) -> usize {
        self.cursor.len(&self.messages)
    }

    /// Returns `true` if there are no unread messages for this cursor.
    pub fn is_empty(&self) -> bool {
        self.cursor.is_empty(&self.messages)
    }

    /// Marks all currently readable messages as seen for this cursor.
    pub fn clear(&mut self) {
        self.cursor.clear(&self.messages);
    }
}

type InternalParam<M> = (
    Local<'static, MessageCursor<M>>,
    ResMut<'static, Messages<M>>,
);

// SAFETY: Delegates state, access declaration, and value fetching to tuple param impl.
unsafe impl<M: Message> SystemParam for MessageMutator<'_, '_, M> {
    type State = <InternalParam<M> as SystemParam>::State;

    type Item<'world, 'state> = MessageMutator<'world, 'state, M>;

    const NON_SEND: bool = false;
    const EXCLUSIVE: bool = false;

    fn init_state(world: &mut crate::world::World) -> Self::State {
        <InternalParam<M> as SystemParam>::init_state(world)
    }

    fn mark_access(table: &mut AccessTable, state: &Self::State) -> bool {
        <InternalParam<M> as SystemParam>::mark_access(table, state)
    }

    unsafe fn build_param<'w, 's>(
        world: crate::world::UnsafeWorld<'w>,
        state: &'s mut Self::State,
        last_run: crate::tick::Tick,
        this_run: crate::tick::Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError> {
        // SAFETY: same world/state/tick contract as delegated tuple parameter.
        let (cursor, messages) = unsafe {
            <InternalParam<M> as SystemParam>::build_param(world, state, last_run, this_run)?
        };

        Ok(MessageMutator { cursor, messages })
    }
}
