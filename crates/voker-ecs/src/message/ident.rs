use core::cmp::Ordering;
use core::fmt::{Debug, Display};
use core::hash::Hash;
use core::marker::PhantomData;

use voker_utils::num::NonMaxU32;

use super::Message;
use crate::utils::DebugName;

// -----------------------------------------------------------------------------
// MessageId

/// A unique identifier for a `Message` type within a specific `World`.
#[derive(Clone, Copy, PartialOrd, Ord)]
#[repr(transparent)]
pub struct MessageId(NonMaxU32);

impl MessageId {
    #[inline]
    pub(crate) const fn new(id: u32) -> Self {
        Self(NonMaxU32::new(id).expect("too many resources"))
    }

    /// Creates a new `MessageId` from a usize.
    ///
    /// # Panics
    /// Panics if `id >= u32::MAX`.
    #[inline(always)]
    pub const fn without_provenance(id: usize) -> Self {
        if id >= u32::MAX as usize {
            core::hint::cold_path();
            panic!("MessageId must be < u32::MAX");
        }
        unsafe { Self(NonMaxU32::new_unchecked(id as u32)) }
    }

    /// Convert `MessageId` to usize.
    #[inline(always)]
    pub const fn index(self) -> usize {
        self.0.get() as usize
    }
}

impl PartialEq for MessageId {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for MessageId {}

impl Hash for MessageId {
    #[inline(always)]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        // Sparse hashing is optimized for smaller values.
        // So we use represented values, rather than the underlying bits
        state.write_u32(self.0.get());
    }
}

impl Debug for MessageId {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.0.get(), f)
    }
}

impl Display for MessageId {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.0.get(), f)
    }
}

// -----------------------------------------------------------------------------
// MessageKey

/// Key for one message in a `MessageQueue<M>` stream.
///
/// `MessageKey` is backed by a wrapping `usize` counter. It is stable for
/// correlation within the stream (for example, tracking ids returned by
/// `write_batch`), but callers should avoid treating it as a globally monotonic
/// timestamp across very long runtimes.
///
/// Ordering is wrap-aware and designed for stream-local comparisons.
pub struct MessageKey<M: Message> {
    index: usize,
    _marker: PhantomData<M>,
}

impl<M: Message> MessageKey<M> {
    #[inline]
    pub(crate) const fn new(index: usize) -> Self {
        Self {
            index,
            _marker: PhantomData,
        }
    }

    #[inline(always)]
    pub const fn without_provenance(index: usize) -> Self {
        Self {
            index,
            _marker: PhantomData,
        }
    }

    /// Convert `MessageKey` to usize.
    #[inline(always)]
    pub const fn index(self) -> usize {
        self.index
    }
}

impl<M: Message> Copy for MessageKey<M> {}

impl<M: Message> Clone for MessageKey<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M: Message> Display for MessageKey<M> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        <Self as Debug>::fmt(self, f)
    }
}

impl<M: Message> Debug for MessageKey<M> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "message<{}>#{}", DebugName::type_name::<M>(), self.index)
    }
}

impl<M: Message> PartialEq for MessageKey<M> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<M: Message> Eq for MessageKey<M> {}

impl<M: Message> PartialOrd for MessageKey<M> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<M: Message> Ord for MessageKey<M> {
    fn cmp(&self, other: &Self) -> Ordering {
        // Non-wrapping difference between two generations after
        // which a signed interpretation becomes negative.
        const DIFF_MAX: usize = usize::MAX >> 1;

        match self.index.wrapping_sub(other.index) {
            0 => Ordering::Equal,
            1..DIFF_MAX => Ordering::Greater,
            _ => Ordering::Less,
        }
    }
}

impl<M: Message> Hash for MessageKey<M> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write_usize(self.index);
    }
}
