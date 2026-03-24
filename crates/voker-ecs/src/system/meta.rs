use core::fmt::Debug;

use bitflags::bitflags;

use crate::system::SystemName;
use crate::tick::Tick;

bitflags! {
    /// Bitflags representing system states and requirements.
    #[derive(Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SystemFlags: u8 {
        /// Set if system cannot be sent across threads
        const NON_SEND = 1 << 0;
        /// Set if system requires exclusive World access
        const EXCLUSIVE = 1 << 1;
    }
}

/// Metadata container for system execution information.
#[derive(Clone, Copy)]
pub struct SystemMeta {
    name: SystemName,
    flags: SystemFlags,
    last_run: Tick,
}

impl Debug for SystemMeta {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SystemMeta")
            .field("name", &self.name)
            .field("last_run", &self.last_run)
            .field("non_send", &self.is_non_send())
            .field("exclusive", &self.is_exclusive())
            .finish()
    }
}

impl SystemMeta {
    #[inline]
    pub fn new(name: SystemName) -> Self {
        Self {
            name,
            flags: SystemFlags::empty(),
            last_run: Tick::new(0),
        }
    }

    #[inline]
    pub fn flags(&self) -> SystemFlags {
        self.flags
    }

    #[inline]
    pub fn name(&self) -> SystemName {
        self.name
    }

    #[inline]
    pub fn get_last_run(&self) -> Tick {
        self.last_run
    }

    #[inline]
    pub fn set_last_run(&mut self, last_run: Tick) {
        self.last_run = last_run;
    }

    #[inline]
    pub fn is_non_send(&self) -> bool {
        self.flags.intersects(SystemFlags::NON_SEND)
    }

    #[inline]
    pub fn is_exclusive(&self) -> bool {
        self.flags.intersects(SystemFlags::EXCLUSIVE)
    }

    #[inline]
    pub fn set_non_send(&mut self) {
        self.flags |= SystemFlags::NON_SEND;
    }

    #[inline]
    pub fn set_exclusive(&mut self) {
        self.flags |= SystemFlags::EXCLUSIVE;
    }
}
