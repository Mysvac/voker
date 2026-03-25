use core::fmt::{Debug, Display};
use core::marker::PhantomData;

use core::panic::Location;

/// A wrapper type that provides [`Location`] information in debug mode.
#[derive(Clone, Copy)]
pub struct DebugLocation(
    PhantomData<&'static Location<'static>>,
    #[cfg(any(debug_assertions, feature = "debug"))] &'static Location<'static>,
);

impl Display for DebugLocation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        #[cfg(any(debug_assertions, feature = "debug"))]
        Display::fmt(self.1, f)?;

        Ok(())
    }
}

impl Debug for DebugLocation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        #[cfg(any(debug_assertions, feature = "debug"))]
        Debug::fmt(self.1, f)?;

        Ok(())
    }
}

impl DebugLocation {
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub const fn caller() -> Self {
        Self(
            PhantomData,
            #[cfg(any(debug_assertions, feature = "debug"))]
            Location::caller(),
        )
    }
}
