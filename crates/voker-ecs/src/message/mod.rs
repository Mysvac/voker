//! Message passing primitives for ECS systems.
//!
//! This module provides a buffered message pipeline.
//!
//! # Important Types
//!
//! - [`Message`]: marker trait for payload types.
//! - [`Messages<M>`]: double-buffered resource storage for one message type.
//! - [`MessageReader<M>`]: read unread messages in systems.
//! - [`MessageMutator<M>`]: mutate unread messages in systems.
//! - [`MessageWriter<M>`]: append messages in systems.
//! - [`MessageId<M>`]: stable id value for correlation in one message stream.
//! - [`MessageCursor<M>`]: per-system read position used by reader/mutator params.
//! - [`MessageRegistry`]: global registry that rotates all message resources in sync.
//!
//! # Lifecycle
//!
//! Each [`Messages<M>`] resource uses two internal sequences:
//! - an older sequence (`messages_a`),
//! - a newer sequence (`messages_b`).
//!
//! Writers append into the newer sequence. When [`MessageRegistry::run_updates`] (or
//! [`World::update_messages`]) runs, the sequences are swapped and the new write
//! sequence is cleared. This gives readers one additional update to observe recent
//! messages while still keeping memory bounded.
//!
//! # Typical Flow
//!
//! 1. Register message type with [`World::register_message`].
//! 2. Write messages through [`MessageWriter`] or [`World::write_message`].
//! 3. Consume unread messages through [`MessageReader`] or [`MessageMutator`].
//! 4. Call [`World::update_messages`] once per update to rotate buffers.
//!
//! [`World::write_message`]: crate::world::World::write_message
//! [`World::update_messages`]: crate::world::World::update_messages
//! [`World::register_message`]: crate::world::World::register_message

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
