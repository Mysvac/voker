//! Event types, storage, lifecycle hooks, and trigger infrastructure.
//!
//! Events are transient data packets broadcast to observers and event readers.
//! This module provides:
//! - the [`Event`] and [`EntityEvent`] traits and their derive re-exports,
//! - built-in lifecycle events (`Add`, `Insert`, `Remove`, `Discard`, `Despawn`, `Clone`),
//! - [`EventId`] for stable event-type identification,
//! - trigger primitives used by the observer dispatch path.
//!
//! Normal usage goes through observers (`On<E>`) or system parameters
//! (`EventReader<E>`, `EventWriter<E>`).

mod event;
mod events;
mod ident;
mod lifecycle;
mod trigger;

pub use event::*;
pub use events::*;
pub use ident::EventId;
pub use lifecycle::*;
pub use trigger::*;

pub use voker_ecs_derive::{EntityEvent, Event};
