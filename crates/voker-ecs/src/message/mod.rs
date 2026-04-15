//! Message passing primitives for ECS systems.
//!
//! This module implements a compact, double-buffered message pipeline used to
//! decouple producers and consumers inside the scheduler. Messages are stored in
//! `MessageQueue<T>` resources and rotated by the `Messages` registry so that
//! writers and readers observe a stable view without unbounded buffering.
//!
//! # Key Types
//!
//! - [`Message`]: marker trait for payload types derived by `Message` proc-macro.
//! - [`MessageQueue<M>`]: the double-buffered resource storing messages for `M`.
//! - [`MessageWriter<M>`]: system parameter for appending messages to the write buffer.
//! - [`MessageReader<M>`]: system parameter for reading unread messages from the read buffer.
//! - [`MessageMutator<M>`]: system parameter for mutating unread messages in place.
//! - [`MessageId`]: compact identifier assigned to each registered message type.
//! - [`Messages`]: global registry holding metadata for all registered message types
//!   and rotating their queues in sync.
//!
//! # Registration & Lifecycle
//!
//! To use a message type `T` you must register it with the world (usually at
//! startup) via `World::register_message::<T>()`. Registration ensures the
//! underlying `MessageQueue<T>` resource exists and records a `MessageId` in the
//! `Messages` registry.
//!
//! Writers append new messages to the write buffer. After a frame/update, call
//! `World::update_messages()` (or allow the schedule plugin to do it) to rotate
//! every registered `MessageQueue<T>`: the write buffer becomes the new read
//! buffer and the old read buffer is cleared. This guarantees that messages
//! written during one update are visible to readers in the following update,
//! while keeping memory usage bounded.
//!
//! # Typical Flow
//!
//! 1. `world.register_message::<T>()` to create the `MessageQueue<T>` resource.
//! 2. In producer systems use `MessageWriter<T>` to append messages.
//! 3. In consumer systems use `MessageReader<T>` / `MessageMutator<T>` to consume.
//! 4. Call `world.update_messages()` once per update to rotate buffers.
//!
//! # Examples
//!
//! See the crate README for high-level examples. A minimal usage pattern:
//!
//! ```no_run
//! use voker_ecs::prelude::*;
//! use voker_ecs_derive::Message;
//!
//! #[derive(Message)]
//! struct EventA { value: u32 }
//!
//! fn producer(mut writer: MessageWriter<EventA>) {
//!     writer.write(EventA { value: 100 });
//! }
//!
//! fn consumer(mut reader: MessageReader<EventA>) {
//!     for m in reader.read() {
//!         let _ = m.value;
//!     }
//! }
//!
//! // At startup:
//! // world.register_message::<EventA>();
//! // After running systems each frame: world.update_messages();
//! ```

// -----------------------------------------------------------------------------
// Modules

mod ident;
mod iterators;
mod message;
mod messages;
mod mutator;
mod queue;
mod reader;
mod writer;

pub use ident::{MessageId, MessageKey};
pub use message::Message;
pub use messages::{MessageMeta, Messages};

pub use voker_ecs_derive::Message;

pub use iterators::{MessageCursor, MessageKeyIter};
pub use iterators::{MessageIterator, MessageWithKeyIter};
pub use iterators::{MessageMutIterator, MessageMutWithKeyIter};
pub use mutator::MessageMutator;
pub use queue::MessageQueue;
pub use reader::MessageReader;
pub use writer::MessageWriter;
