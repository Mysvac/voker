//! Observer subsystem.
//!
//! This module defines:
//! - observer system input ([`On`]),
//! - observer construction traits ([`IntoObserver`], [`IntoEntityObserver`]),
//! - runtime observer metadata ([`Observer`], [`ObserverId`]),
//! - and cached dispatch indices ([`CachedObservers`]).
//!
//! Most users interact with observers through world/commands helper APIs such
//! as `add_observer` and `observe`.

mod ident;
mod system;

mod builder;

pub use ident::{ObservedBy, ObserverId};
pub use system::{IntoObserverSystem, ObserverSystem, On};

mod observer;
mod observers;

pub use builder::*;
pub use observer::*;
pub use observers::*;
