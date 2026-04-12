use core::fmt::Debug;
use core::iter::{Chain, FusedIterator};
use core::marker::PhantomData;
use core::slice::{Iter, IterMut};

use crate::world::{FromWorld, World};

use super::ident::MessageId;
use super::{Message, Messages};

// -----------------------------------------------------------------------------
// MessageIdIter

/// Iterator over [`MessageId`] values written by a batch call.
///
/// This iterator yields ids in write order: `[start, end)`.
///
/// # Example
///
/// ```rust
/// use voker_ecs::message::{Message, Messages};
///
/// #[derive(Message)]
/// struct Event;
///
/// let mut messages = Messages::<Event>::default();
/// let mut ids = messages.write_batch([Event, Event]);
///
/// assert_eq!(ids.next().map(|id| id.index()), Some(0));
/// assert_eq!(ids.next().map(|id| id.index()), Some(1));
/// assert_eq!(ids.next(), None);
/// ```
pub struct MessageIdIter<M: Message> {
    pub(super) last: usize,
    pub(super) end: usize,
    pub(super) _marker: PhantomData<M>,
}

impl<M: Message> Iterator for MessageIdIter<M> {
    type Item = MessageId<M>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.last == self.end {
            return None;
        }

        let id = MessageId::new(self.last);
        self.last = self.last.wrapping_add(1);
        Some(id)
    }

    fn count(self) -> usize {
        self.end.wrapping_sub(self.last)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.end.wrapping_sub(self.last);
        (len, Some(len))
    }
}

impl<M: Message> ExactSizeIterator for MessageIdIter<M> {}
impl<M: Message> FusedIterator for MessageIdIter<M> {}

// -----------------------------------------------------------------------------
// MessageWithIdIter

/// Per-system read position for one `Messages<M>` stream.
///
/// `MessageCursor` is usually managed by ECS as a local system parameter state
/// (see [`crate::message::MessageReader`] and [`crate::message::MessageMutator`]).
/// Each system instance has its own cursor, so one system reading messages does
/// not consume them for other systems.
///
/// Cursor advancement is pull-based: it advances when iterator items are
/// consumed (or when methods like `count`/`nth`/`last` skip items).
pub struct MessageCursor<M: Message> {
    pub(super) last_index: usize,
    pub(super) _marker: PhantomData<M>,
}

impl<M: Message> FromWorld for MessageCursor<M> {
    fn from_world(world: &World) -> Self {
        let last_index = world
            .get_resource::<Messages<M>>()
            .map(Messages::<M>::oldest_message_index)
            .unwrap_or_default();

        Self {
            last_index,
            _marker: PhantomData,
        }
    }
}

impl<M: Message> Debug for MessageCursor<M> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("MessageCursor").field(&self.last_index).finish()
    }
}

impl<M: Message> Copy for MessageCursor<M> {}

impl<M: Message> Clone for MessageCursor<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M: Message> MessageCursor<M> {
    /// Returns unread count for this cursor in the given message storage.
    pub fn len(&self, messages: &Messages<M>) -> usize {
        let upper = messages.counter.wrapping_sub(self.last_index);
        upper.min(messages.len())
    }

    /// Returns `true` if this cursor has no unread messages.
    pub fn is_empty(&self, messages: &Messages<M>) -> bool {
        messages.is_empty() || messages.counter == self.last_index
    }

    /// Marks all currently readable messages as consumed for this cursor.
    pub fn clear(&mut self, messages: &Messages<M>) {
        self.last_index = messages.counter;
    }

    /// Reads unread messages and advances the cursor as items are consumed.
    pub fn read<'a>(&'a mut self, messages: &'a Messages<M>) -> MessageIterator<'a, M> {
        MessageWithIdIterator::new(self, messages).without_id()
    }

    /// Reads unread messages with ids and advances the cursor as items are consumed.
    pub fn read_with_id<'a>(
        &'a mut self,
        messages: &'a Messages<M>,
    ) -> MessageWithIdIterator<'a, M> {
        MessageWithIdIterator::new(self, messages)
    }

    /// Reads unread messages mutably and advances the cursor as items are consumed.
    pub fn read_mut<'a>(&'a mut self, messages: &'a mut Messages<M>) -> MessageMutIterator<'a, M> {
        MessageMutWithIdIterator::new(self, messages).without_id()
    }

    /// Reads unread mutable messages with ids and advances the cursor.
    pub fn read_mut_with_id<'a>(
        &'a mut self,
        messages: &'a mut Messages<M>,
    ) -> MessageMutWithIdIterator<'a, M> {
        MessageMutWithIdIterator::new(self, messages)
    }
}

// -----------------------------------------------------------------------------
// MessageWithIdIterator

/// Iterator over unread messages with their message IDs.
///
/// Created by [`MessageCursor::read_with_id`] and
/// [`crate::message::MessageReader::read_with_id`].
///
/// Consuming this iterator advances the underlying cursor.
#[derive(Debug)]
pub struct MessageWithIdIterator<'a, M: Message> {
    cursor: &'a mut MessageCursor<M>,
    chain: Chain<Iter<'a, (MessageId<M>, M)>, Iter<'a, (MessageId<M>, M)>>,
    unread: usize,
}

impl<'a, M: Message> MessageWithIdIterator<'a, M> {
    fn new(cursor: &'a mut MessageCursor<M>, messages: &'a Messages<M>) -> Self {
        let unread = cursor.len(messages);
        let a_index = cursor.last_index.wrapping_sub(messages.messages_a.start_id);
        let b_index = cursor.last_index.wrapping_sub(messages.messages_b.start_id);

        let a = messages.messages_a.get(a_index..).unwrap_or_default();
        let b = messages.messages_b.get(b_index..).unwrap_or_default();
        debug_assert_eq!(unread, a.len() + b.len());

        cursor.last_index = messages.counter.wrapping_sub(unread);

        Self {
            cursor,
            chain: a.iter().chain(b.iter()),
            unread,
        }
    }

    pub fn without_id(self) -> MessageIterator<'a, M> {
        MessageIterator { iter: self }
    }
}

impl<'a, M: Message> Iterator for MessageWithIdIterator<'a, M> {
    type Item = (MessageId<M>, &'a M);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.chain.next() {
            self.cursor.last_index = self.cursor.last_index.wrapping_add(1);
            self.unread = self.unread.saturating_sub(1);
            Some((item.0, &item.1))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.unread, Some(self.unread))
    }

    fn count(self) -> usize {
        self.cursor.last_index = self.cursor.last_index.wrapping_add(self.unread);
        self.unread
    }

    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        let instance = self.chain.last()?;
        self.cursor.last_index = self.cursor.last_index.wrapping_add(self.unread);
        Some((instance.0, &instance.1))
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        if let Some(instance) = self.chain.nth(n) {
            let advanced = n.saturating_add(1);
            self.cursor.last_index = self.cursor.last_index.wrapping_add(advanced);
            self.unread = self.unread.saturating_sub(advanced);
            Some((instance.0, &instance.1))
        } else {
            self.cursor.last_index = self.cursor.last_index.wrapping_add(self.unread);
            self.unread = 0;
            None
        }
    }
}

impl<M: Message> ExactSizeIterator for MessageWithIdIterator<'_, M> {}
impl<M: Message> FusedIterator for MessageWithIdIterator<'_, M> {}

// -----------------------------------------------------------------------------
// MessageIterator

/// Iterator over unread messages.
///
/// This is the id-stripped form of [`MessageWithIdIterator`].
///
/// # Example
///
/// ```rust
/// use voker_ecs::prelude::*;
///
/// #[derive(Message)]
/// struct Hit;
///
/// fn read_hits(mut reader: MessageReader<Hit>) {
///     for _ in reader.read() {}
/// }
/// ```
#[derive(Debug)]
pub struct MessageIterator<'a, M: Message> {
    iter: MessageWithIdIterator<'a, M>,
}

impl<'a, M: Message> Iterator for MessageIterator<'a, M> {
    type Item = &'a M;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(_, m)| m)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    fn count(self) -> usize {
        self.iter.count()
    }

    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        self.iter.last().map(|(_, m)| m)
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.iter.nth(n).map(|(_, m)| m)
    }
}

impl<M: Message> ExactSizeIterator for MessageIterator<'_, M> {}
impl<M: Message> FusedIterator for MessageIterator<'_, M> {}

// -----------------------------------------------------------------------------
// MessageMutWithIdIterator

/// Iterator over unread messages with their message IDs.
///
/// Mutable counterpart of [`MessageWithIdIterator`].
///
/// Created by [`MessageCursor::read_mut_with_id`] and
/// [`crate::message::MessageMutator::read_with_id`].
///
/// Consuming this iterator advances the underlying cursor.
#[derive(Debug)]
pub struct MessageMutWithIdIterator<'a, M: Message> {
    cursor: &'a mut MessageCursor<M>,
    chain: Chain<IterMut<'a, (MessageId<M>, M)>, IterMut<'a, (MessageId<M>, M)>>,
    unread: usize,
}

impl<'a, M: Message> MessageMutWithIdIterator<'a, M> {
    fn new(cursor: &'a mut MessageCursor<M>, messages: &'a mut Messages<M>) -> Self {
        let unread = cursor.len(messages);
        let a_index = cursor.last_index.wrapping_sub(messages.messages_a.start_id);
        let b_index = cursor.last_index.wrapping_sub(messages.messages_b.start_id);

        let a = messages.messages_a.get_mut(a_index..).unwrap_or_default();
        let b = messages.messages_b.get_mut(b_index..).unwrap_or_default();
        debug_assert_eq!(unread, a.len() + b.len());

        cursor.last_index = messages.counter.wrapping_sub(unread);

        Self {
            cursor,
            chain: a.iter_mut().chain(b.iter_mut()),
            unread,
        }
    }

    pub fn without_id(self) -> MessageMutIterator<'a, M> {
        MessageMutIterator { iter: self }
    }
}

impl<'a, M: Message> Iterator for MessageMutWithIdIterator<'a, M> {
    type Item = (MessageId<M>, &'a mut M);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.chain.next() {
            self.cursor.last_index = self.cursor.last_index.wrapping_add(1);
            self.unread = self.unread.saturating_sub(1);
            Some((item.0, &mut item.1))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.unread, Some(self.unread))
    }

    fn count(self) -> usize {
        self.cursor.last_index = self.cursor.last_index.wrapping_add(self.unread);
        self.unread
    }

    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        let instance = self.chain.last()?;
        self.cursor.last_index = self.cursor.last_index.wrapping_add(self.unread);
        Some((instance.0, &mut instance.1))
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        if let Some(instance) = self.chain.nth(n) {
            let advanced = n.saturating_add(1);
            self.cursor.last_index = self.cursor.last_index.wrapping_add(advanced);
            self.unread = self.unread.saturating_sub(advanced);
            Some((instance.0, &mut instance.1))
        } else {
            self.cursor.last_index = self.cursor.last_index.wrapping_add(self.unread);
            self.unread = 0;
            None
        }
    }
}

impl<M: Message> ExactSizeIterator for MessageMutWithIdIterator<'_, M> {}
impl<M: Message> FusedIterator for MessageMutWithIdIterator<'_, M> {}

// -----------------------------------------------------------------------------
// MessageWithIdIterator

/// Iterator over unread mutable messages.
///
/// This is the id-stripped form of [`MessageMutWithIdIterator`].
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
/// fn clamp(mut mutator: MessageMutator<Damage>) {
///     for msg in mutator.read() {
///         msg.amount = msg.amount.min(100);
///     }
/// }
/// ```
#[derive(Debug)]
pub struct MessageMutIterator<'a, M: Message> {
    iter: MessageMutWithIdIterator<'a, M>,
}

impl<'a, M: Message> Iterator for MessageMutIterator<'a, M> {
    type Item = &'a mut M;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(_, m)| m)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    fn count(self) -> usize {
        self.iter.count()
    }

    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        self.iter.last().map(|(_, m)| m)
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.iter.nth(n).map(|(_, m)| m)
    }
}

impl<M: Message> ExactSizeIterator for MessageMutIterator<'_, M> {}
impl<M: Message> FusedIterator for MessageMutIterator<'_, M> {}
