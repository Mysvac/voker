use crate::{
    borrow::ResMut,
    message::{Message, MessageId, MessageIdIterator, Messages},
    system::SystemParam,
};

/// System parameter that appends messages of type `M`.
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
/// }
/// ```
pub struct MessageWriter<'w, M: Message> {
    messages: ResMut<'w, Messages<M>>,
}

impl<'w, M: Message> MessageWriter<'w, M> {
    /// Writes one message and returns its generated id.
    pub fn write(&mut self, message: M) -> MessageId<M> {
        self.messages.write(message)
    }

    /// Writes a batch of messages and returns the generated id range.
    pub fn write_batch(&mut self, messages: impl IntoIterator<Item = M>) -> MessageIdIterator<M> {
        self.messages.write_batch(messages)
    }
}

type InternalParam<M> = ResMut<'static, Messages<M>>;

// SAFETY: Delegates state, access declaration, and value fetching to `ResMut<Messages<M>>`.
unsafe impl<M: Message> SystemParam for MessageWriter<'_, M> {
    type State = <InternalParam<M> as SystemParam>::State;
    type Item<'world, 'state> = MessageWriter<'world, M>;

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
        // SAFETY: same world/state/tick contract as delegated parameter.
        let messages = unsafe {
            <InternalParam<M> as SystemParam>::build_param(world, state, last_run, this_run)?
        };

        Ok(MessageWriter { messages })
    }
}
