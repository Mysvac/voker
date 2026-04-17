use alloc::vec::Vec;
use core::any::TypeId;
use core::fmt::Debug;

use voker_utils::extra::TypeIdMap;

use super::{Event, EventId};
use crate::utils::DebugName;

// -----------------------------------------------------------------------------
// EventMeta

/// Compact runtime metadata for a registered event type.
pub struct EventMeta {
    id: EventId,
    name: DebugName,
    // type_id: TypeId,
}

impl Debug for EventMeta {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Event")
            .field("id", &self.id)
            .field("name", &self.name)
            .finish()
    }
}

impl EventMeta {
    /// Returns the event's unique ID.
    #[inline(always)]
    pub fn id(&self) -> EventId {
        self.id
    }

    /// Returns the event's debug name.
    #[inline(always)]
    pub fn name(&self) -> DebugName {
        self.name
    }

    // /// Returns the event's [`TypeId`].
    // #[inline(always)]
    // pub fn type_id(&self) -> TypeId {
    //     self.type_id
    // }
}

// -----------------------------------------------------------------------------
// Lifecycle

pub const ADD: EventId = EventId::new(0);
pub const CLONE: EventId = EventId::new(1);
pub const INSERT: EventId = EventId::new(2);
pub const REMOVE: EventId = EventId::new(3);
pub const DISCARD: EventId = EventId::new(4);
pub const DESPAWN: EventId = EventId::new(5);

// -----------------------------------------------------------------------------
// EventMeta

/// Registry of all registered event types and their metadata.
pub struct Events {
    metas: Vec<EventMeta>,
    mapper: TypeIdMap<EventId>,
}

impl Debug for Events {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.metas, f)
    }
}

impl Events {
    /// Creates a new empty event registry.
    #[inline]
    pub(crate) fn new() -> Self {
        let mut this = Self {
            metas: Vec::with_capacity(8),
            mapper: TypeIdMap::with_capacity(8),
        };

        this.register::<super::Add>();
        this.register::<super::Clone>();
        this.register::<super::Insert>();
        this.register::<super::Remove>();
        this.register::<super::Discard>();
        this.register::<super::Despawn>();

        assert_eq!(
            this.get_id(TypeId::of::<super::Add>()),
            Some(EventId::new(0))
        );
        assert_eq!(
            this.get_id(TypeId::of::<super::Clone>()),
            Some(EventId::new(1))
        );
        assert_eq!(
            this.get_id(TypeId::of::<super::Insert>()),
            Some(EventId::new(2))
        );
        assert_eq!(
            this.get_id(TypeId::of::<super::Remove>()),
            Some(EventId::new(3))
        );
        assert_eq!(
            this.get_id(TypeId::of::<super::Discard>()),
            Some(EventId::new(4))
        );
        assert_eq!(
            this.get_id(TypeId::of::<super::Despawn>()),
            Some(EventId::new(5))
        );

        this
    }

    /// Register event type `T`.
    pub(crate) fn register<T: Event>(&mut self) -> EventId {
        if let Some(id) = self.mapper.get(TypeId::of::<T>()) {
            return *id;
        }
        let id = EventId::new(self.metas.len() as u32);
        let meta = EventMeta {
            id,
            name: DebugName::type_name::<T>(),
            // type_id: TypeId::of::<T>(),
        };
        self.metas.push(meta);
        self.mapper.insert(TypeId::of::<T>(), id);
        id
    }
}

impl Events {
    /// Returns the number of registered event types.
    #[inline]
    #[expect(clippy::len_without_is_empty, reason = "useless")]
    pub const fn len(&self) -> usize {
        self.metas.len()
    }

    /// Looks up a event ID by its [`TypeId`].
    #[inline]
    pub fn get_id(&self, type_id: TypeId) -> Option<EventId> {
        self.mapper.get(type_id).copied()
    }

    /// Returns the event info for the given ID.
    #[inline]
    pub fn get(&self, id: EventId) -> Option<&EventMeta> {
        self.metas.get(id.index())
    }

    /// Returns the event info for the given ID without bounds checking.
    ///
    /// # Safety
    /// The caller must ensure `id` is a valid ID (i.e., `id.index() < self.len()`).
    #[inline]
    pub unsafe fn get_unchecked(&self, id: EventId) -> &EventMeta {
        debug_assert!(id.index() < self.metas.len());
        unsafe { self.metas.get_unchecked(id.index()) }
    }

    /// Extracts a slice containing the entire event metadata registry.
    #[inline]
    pub fn as_slice(&self) -> &[EventMeta] {
        self.metas.as_slice()
    }

    /// Returns an iterator over the `EventMeta` values.
    #[inline]
    pub fn iter(&self) -> core::slice::Iter<'_, EventMeta> {
        self.metas.iter()
    }
}
