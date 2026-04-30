use core::fmt::Debug;

use bitflags::bitflags;
#[cfg(feature = "trace")]
use voker_utils::debug::DebugName;

use crate::system::SystemId;
use crate::tick::Tick;

bitflags! {
    /// Bitflags representing system states and requirements.
    #[derive(Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SystemFlags: u8 {
        /// Set if system need to apply deferred commands.
        const NO_OP = 1 << 0;
        /// Set if system need to apply deferred commands.
        const DEFERRED = 1 << 1;
        /// Set if system cannot be sent across threads
        const NON_SEND = 1 << 2;
        /// Set if system requires exclusive World access
        const EXCLUSIVE = 1 << 3;
    }
}

/// Metadata container for system execution information.
#[derive(Clone)]
pub struct SystemMeta {
    pub(crate) id: SystemId,
    pub(crate) flags: SystemFlags,
    pub(crate) last_run: Tick,
    #[cfg(feature = "trace")]
    pub(crate) system_span: tracing::Span,
}

impl Debug for SystemMeta {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SystemMeta")
            .field("id", &self.id)
            .field("last_run", &self.last_run)
            .field("deferred", &self.is_deferred())
            .field("non_send", &self.is_non_send())
            .field("exclusive", &self.is_exclusive())
            .finish()
    }
}

impl SystemMeta {
    #[inline]
    pub(crate) fn new<T: 'static>() -> Self {
        #[cfg(feature = "trace")]
        let name = DebugName::type_name::<T>();
        Self {
            id: SystemId::of::<T>(),
            flags: SystemFlags::empty(),
            last_run: Tick::new(0),
            #[cfg(feature = "trace")]
            system_span: tracing::info_span!(parent: None, "system", name = name.parse()),
        }
    }

    #[inline]
    pub const fn id(&self) -> SystemId {
        self.id
    }

    #[inline]
    pub const fn flags(&self) -> SystemFlags {
        self.flags
    }

    #[inline]
    pub const fn last_run(&self) -> Tick {
        self.last_run
    }

    #[inline]
    pub const fn set_last_run(&mut self, last_run: Tick) {
        self.last_run = last_run;
    }

    #[inline]
    pub const fn is_no_op(&self) -> bool {
        self.flags.intersects(SystemFlags::NO_OP)
    }

    #[inline]
    pub const fn is_deferred(&self) -> bool {
        self.flags.intersects(SystemFlags::DEFERRED)
    }

    #[inline]
    pub const fn is_non_send(&self) -> bool {
        self.flags.intersects(SystemFlags::NON_SEND)
    }

    #[inline]
    pub const fn is_exclusive(&self) -> bool {
        self.flags.intersects(SystemFlags::EXCLUSIVE)
    }

    #[inline]
    pub const fn set_no_op(&mut self) {
        self.flags = self.flags.union(SystemFlags::NO_OP);
    }

    #[inline]
    pub const fn set_deferred(&mut self) {
        self.flags = self.flags.union(SystemFlags::DEFERRED);
    }

    #[inline]
    pub const fn set_non_send(&mut self) {
        self.flags = self.flags.union(SystemFlags::NON_SEND);
    }

    #[inline]
    pub const fn set_exclusive(&mut self) {
        self.flags = self.flags.union(SystemFlags::EXCLUSIVE);
    }
}
