#![expect(clippy::module_inception, reason = "For better structure.")]

/// Marker trait for ECS message payload types.
///
/// A `Message` type is a short-lived payload sent between systems through
/// [`MessageQueue<T>`]. The trait has no methods: it only encodes bounds required
/// by message storage and cross-system usage.
///
/// For user code, the recommended path is `#[derive(Message)]`.
///
/// To participate in automatic lifecycle rotation, register the type with
/// [`World::register_message`] and run [`World::update_messages`] each update.
///
/// # Using MessageQueue In World
///
/// ```rust
/// use voker_ecs::prelude::*;
///
/// #[derive(Message)]
/// struct Collision { /* .. */ }
///
/// let mut world = World::alloc();
/// world.register_message::<Collision>();
///
/// world.write_message(Collision { /* .. */ });
///
/// world.update_messages();
/// ```
///
/// # Using MessageQueue In Systems
///
/// `Message` is consumed through system parameters in three roles:
/// - [`MessageWriter<T>`]: append new messages.
/// - [`MessageReader<T>`]: read unread messages immutably.
/// - [`MessageMutator<T>`]: read unread messages mutably.
///
/// `MessageReader` and `MessageMutator` each keep an independent local cursor,
/// so one system reading messages does not consume them for another system.
///
/// ```rust
/// use voker_ecs::prelude::*;
///
/// #[derive(Message)]
/// struct Damage {
///     amount: u32,
/// }
///
/// fn emit(mut writer: MessageWriter<Damage>) {
///     writer.write(Damage { amount: 120 });
/// }
///
/// fn clamp(mut mutator: MessageMutator<Damage>) {
///     for msg in mutator.read() {
///         msg.amount = msg.amount.min(100);
///     }
/// }
///
/// fn log(mut reader: MessageReader<Damage>) {
///     for msg in reader.read() {
///         let _ = msg.amount;
///     }
/// }
/// ```
///
/// [`MessageQueue<T>`]: crate::message::MessageQueue
/// [`MessageWriter<T>`]: crate::message::MessageWriter
/// [`MessageReader<T>`]: crate::message::MessageReader
/// [`MessageMutator<T>`]: crate::message::MessageMutator
/// [`World::register_message`]: crate::world::World::register_message
/// [`World::update_messages`]: crate::world::World::update_messages
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a message",
    label = "invalid message",
    note = "Consider annotating `{Self}` with `#[derive(Message)]`."
)]
pub trait Message: Send + Sync + 'static {}
