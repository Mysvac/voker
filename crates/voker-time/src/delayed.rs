use alloc::vec::Vec;
use core::fmt::Debug;
use core::time::Duration;
use voker_ecs::reflect::ReflectResource;
use voker_ecs::world::World;
use voker_reflect::Reflect;

use voker_ecs::borrow::{Res, ResMut};
use voker_ecs::command::{CommandQueue, Commands};
use voker_ecs::resource::Resource;
use voker_utils::hash::HashMap;

use crate::Time;

// -----------------------------------------------------------------------------
// DelayedCommandQueue

/// A delayed command queue that should be submitted at `submit_at`.
#[derive(Reflect, Debug)]
pub struct DelayedCommandQueue {
    pub submit_at: Duration,
    #[reflect(ignore, default)]
    pub queue: CommandQueue,
}

// -----------------------------------------------------------------------------
// DelayedCommands

/// A wrapper over [`Commands`] that stores delayed [`CommandQueue`] values.
///
/// Queues are deduplicated by delay duration. On drop, each queue is converted
/// into a [`DelayedCommandQueue`] and stored in [`DelayedCommandQueues`].
pub struct DelayedCommands<'w, 's> {
    queues: HashMap<Duration, CommandQueue>,
    commands: Commands<'w, 's>,
}

impl<'w, 's> DelayedCommands<'w, 's> {
    /// Returns a [`Commands`] writer whose queued commands will be delayed by `duration`.
    #[must_use = "The returned Commands must be used to submit commands with this delay."]
    pub fn duration(&mut self, duration: Duration) -> Commands<'w, '_> {
        let queue = self.queues.entry(duration).or_default();
        self.commands.rebound_to(queue)
    }

    /// Returns a [`Commands`] writer whose queued commands will be delayed by `secs` seconds.
    #[inline]
    #[must_use = "The returned Commands must be used to submit commands with this delay."]
    pub fn secs(&mut self, secs: f32) -> Commands<'w, '_> {
        self.duration(Duration::from_secs_f32(secs))
    }

    fn submit(&mut self) {
        let queues = self
            .queues
            .drain()
            .map(|(submit_at, queue)| DelayedCommandQueue { submit_at, queue })
            .collect::<Vec<_>>();

        self.commands.queue(move |world: &mut World| {
            let elapsed = world.resource::<Time>().elapsed();
            let mut delayed_queues = world.resource_mut_or_init::<DelayedCommandQueues>();

            for mut delayed in queues {
                delayed.submit_at += elapsed;
                delayed_queues.queues.push(delayed);
            }
        });
    }
}

/// Extension trait for [`Commands`] that provides delayed command helpers.
pub trait DelayedCommandsExt {
    /// Returns a helper that can queue commands for delayed execution.
    fn delayed(&mut self) -> DelayedCommands<'_, '_>;
}

impl<'w, 's> DelayedCommandsExt for Commands<'w, 's> {
    fn delayed(&mut self) -> DelayedCommands<'_, '_> {
        DelayedCommands {
            commands: self.reborrow(),
            queues: HashMap::default(),
        }
    }
}

impl<'w, 's> Drop for DelayedCommands<'w, 's> {
    fn drop(&mut self) {
        self.submit();
    }
}

// -----------------------------------------------------------------------------
// DelayedCommandQueues

/// Resource that stores delayed command queues.
#[derive(Reflect, Resource, Default)]
#[reflect(NotCloneable, Default, Debug)]
#[type_data(ReflectResource)]
pub struct DelayedCommandQueues {
    pub(crate) queues: Vec<DelayedCommandQueue>,
}

impl Debug for DelayedCommandQueues {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("DelayedCommandQueues")
            .field(&self.queues.as_slice())
            .finish()
    }
}

/// Appends command queues that reached their due time.
pub fn check_delayed_command_queues(
    mut queues: ResMut<DelayedCommandQueues>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let elapsed = time.elapsed();

    let mut index = 0;
    while index < queues.queues.len() {
        if queues.queues[index].submit_at <= elapsed {
            let mut delayed_queue = queues.queues.swap_remove(index);
            commands.append(&mut delayed_queue.queue);
        } else {
            index += 1;
        }
    }
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
#[expect(clippy::print_stdout, reason = "Allowed in tests.")]
mod tests {
    use core::time::Duration;

    use voker_app::{App, Startup};
    use voker_ecs::command::Commands;
    use voker_ecs::derive::Component;

    use crate::{DelayedCommandsExt, TimePlugin, TimeUpdateStrategy};

    #[derive(Component, Clone)]
    struct DummyComponent;

    fn dummy_count(app: &mut App) -> usize {
        let world = app.world_mut();
        let query = world.query::<&DummyComponent>();
        query.into_iter().count()
    }

    #[test]
    fn delayed_queues_should_run_with_time_plugin_enabled() {
        fn queue_commands(mut commands: Commands) {
            commands.delayed().secs(0.1).spawn(DummyComponent);
            commands.spawn(DummyComponent);

            let mut delayed_cmds = commands.delayed();
            delayed_cmds.secs(0.5).spawn(DummyComponent);

            let mut in_1_sec = delayed_cmds.duration(Duration::from_secs_f32(1.0));
            in_1_sec.spawn(DummyComponent);
            in_1_sec.spawn(DummyComponent);
            in_1_sec.spawn(DummyComponent);
        }

        let mut app = App::new();
        app.add_plugins(TimePlugin)
            .add_systems(Startup, queue_commands)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
                0.2,
            )));

        for frame in 0..10 {
            app.update();
            let count = dummy_count(&mut app);

            match frame {
                0 => assert_eq!(count, 1),
                1 | 2 => assert_eq!(count, 2),
                3 | 4 => assert_eq!(count, 3),
                _ => assert_eq!(count, 6),
            }
        }
    }

    #[test]
    fn delayed_same_duration_should_release_together() {
        fn queue_commands(mut commands: Commands) {
            commands.spawn(DummyComponent);

            let mut delayed = commands.delayed();
            delayed.secs(0.5).spawn(DummyComponent);
            delayed.secs(0.5).spawn(DummyComponent);
        }

        let mut app = App::new();
        app.add_plugins(TimePlugin)
            .add_systems(Startup, queue_commands)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
                0.2,
            )));

        app.update();
        assert_eq!(dummy_count(&mut app), 1);

        app.update();
        assert_eq!(dummy_count(&mut app), 1);

        app.update();
        assert_eq!(dummy_count(&mut app), 1);

        app.update();
        assert_eq!(dummy_count(&mut app), 3);
    }
}
