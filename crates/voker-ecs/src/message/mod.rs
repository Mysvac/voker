//! Message passing primitives for ECS systems.
//!
//! This module provides a buffered message pipeline with three primary roles:
//! - [`MessageWriter`] appends new messages.
//! - [`MessageReader`] and [`MessageMutator`] consume unread messages through a local cursor.
//! - [`MessageRegistry`] rotates all registered [`Messages`] storages together.
//!
//! # Lifecycle
//!
//! Each [`Messages<M>`] resource uses two internal sequences:
//! - an older sequence (`messages_a`),
//! - a newer sequence (`messages_b`).
//!
//! Writers append into the newer sequence. When [`MessageRegistry::run_updates`] (or
//! [`crate::world::World::update_messages`]) runs, the sequences are swapped and the
//! new write sequence is cleared. This gives readers one additional update to observe
//! recent messages while still keeping memory bounded.

// -----------------------------------------------------------------------------
// Modules

mod ident;
mod iterators;
mod messages;
mod mutator;
mod reader;
mod registry;
mod writer;

pub use voker_ecs_derive::Message;

pub use ident::{Message, MessageId};
pub use iterators::{MessageCursor, MessageIdIter};
pub use iterators::{MessageIterator, MessageWithIdIterator};
pub use iterators::{MessageMutIterator, MessageMutWithIdIterator};
pub use messages::Messages;
pub use mutator::MessageMutator;
pub use reader::MessageReader;
pub use registry::MessageRegistry;
pub use writer::MessageWriter;
