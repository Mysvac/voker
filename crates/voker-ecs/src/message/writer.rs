use crate::borrow::ResMut;
use crate::message::{Message, MessageKey, MessageKeyIter, MessageQueue};
use crate::system::{AccessTable, SystemParam, SystemParamError};
use crate::world::{UnsafeWorld, World};

/// System parameter that appends messages of type `M`.
///
/// MessageQueue are appended into the current write sequence of [`MessageQueue<M>`]
/// and become readable according to message lifecycle rotation.
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
/// fn detect_collisions(mut writer: MessageWriter<Collision>) {
///     writer.write(Collision { lhs: 1, rhs: 2 });
///
///     writer.write_batch([
///         Collision { lhs: 10, rhs: 11 },
///         Collision { lhs: 20, rhs: 21 },
///     ]);
/// }
/// ```
pub struct MessageWriter<'w, M: Message> {
    messages: ResMut<'w, MessageQueue<M>>,
}

impl<'w, M: Message> MessageWriter<'w, M> {
    /// Writes one message through it's default value and returns its generated id.
    #[inline]
    pub fn write_default(&mut self) -> MessageKey<M>
    where
        M: Default,
    {
        self.messages.write(M::default())
    }

    /// Writes one message and returns its generated id.
    #[inline]
    pub fn write(&mut self, message: M) -> MessageKey<M> {
        self.messages.write(message)
    }

    /// Writes a batch of messages and returns the generated id range.
    #[inline]
    pub fn write_batch(&mut self, messages: impl IntoIterator<Item = M>) -> MessageKeyIter<M> {
        self.messages.write_batch(messages)
    }
}

type InternalParam<M> = ResMut<'static, MessageQueue<M>>;

// SAFETY: Delegates state, access declaration, and value fetching to `ResMut<MessageQueue<M>>`.
unsafe impl<M: Message> SystemParam for MessageWriter<'_, M> {
    type State = <InternalParam<M> as SystemParam>::State;
    type Item<'world, 'state> = MessageWriter<'world, M>;

    const NON_SEND: bool = false;
    const EXCLUSIVE: bool = false;

    fn init_state(world: &mut World) -> Self::State {
        <InternalParam<M> as SystemParam>::init_state(world)
    }

    fn mark_access(table: &mut AccessTable, state: &Self::State) -> bool {
        <InternalParam<M> as SystemParam>::mark_access(table, state)
    }

    unsafe fn build_param<'w, 's>(
        world: UnsafeWorld<'w>,
        state: &'s mut Self::State,
        last_run: crate::tick::Tick,
        this_run: crate::tick::Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError> {
        // SAFETY: same world/state/tick contract as delegated parameter.
        let messages = unsafe {
            <InternalParam<M> as SystemParam>::build_param(world, state, last_run, this_run)?
        };

        Ok(MessageWriter { messages })
    }
}
