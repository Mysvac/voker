
# ECS

> README is modified from bevy_ecs.

ECS is a software pattern that involves breaking your program up into Entities, Components, and Systems.
Entities are unique "things" that are assigned groups of Components, which are then processed using Systems.

For example, one entity might have a `Position` and `Velocity` component, whereas another entity might have
a `Position` and `UI` component. You might have a movement system that runs on all entities with a Position
and Velocity component.

The ECS pattern encourages clean, decoupled designs by forcing you to break up your app data and logic into
its core components. It also helps make your code faster by optimizing memory access patterns and making parallelism easier.

This README covers core ECS building blocks first, then highlights practical feature modules.

## Worlds

Entities, Components, and Resources are stored in a `World`.
Worlds, much like `std::collections`'s `HashSet` and `Vec`, expose operations to insert, read, write, and remove the data they store.

```rust
use voker_ecs::world::World;

let world = World::alloc();
```

## Components

Components are normal Rust structs. They are data stored in a `World` and specific instances of Components correlate to Entities.

```rust
use voker_ecs::prelude::*;

#[derive(Component)]
struct Position { x: f32, y: f32 }
```

## Resources

Apps often require unique resources, such as asset collections, renderers, audio servers, time, etc.
voker ECS makes this pattern a first class citizen. `Resource` is a special kind of component that
does not belong to any entity. Instead, it is identified uniquely by its type:

```rust
use voker_ecs::prelude::*;

#[derive(Resource)]
struct Time { seconds: f32 }
```

## Entities

Entities are unique identifiers that correlate to zero or more Components.

```rust
use voker_ecs::prelude::*;

#[derive(Component)]
struct Position { x: f32, y: f32 }
#[derive(Component)]
struct Velocity { x: f32, y: f32 }

let mut world = World::alloc();

let entity: EntityOwned = world
    .spawn((Position { x: 0.0, y: 0.0 }, Velocity { x: 1.0, y: 0.0 }));

let position = entity.get::<Position>().unwrap();
let velocity = entity.get::<Velocity>().unwrap();
```

## Systems

Systems are normal Rust functions. Thanks to the Rust type system, voker ECS can use function parameter types
to determine what data needs to be sent to the system. It also uses this "data access" information to determine what
Systems can run in parallel with each other.

```rust
use voker_ecs::prelude::*;

#[derive(Component)]
struct Position { x: f32, y: f32 }

#[derive(Resource)]
struct Time { seconds: f32 }

fn print_position(query: Query<(Entity, &Position)>, time: Res<Time>) {
    for (entity, position) in query {
        println!(
            "|{}| Entity {} is at position: x {}, y {}",
            time.seconds, entity, position.x, position.y,
        );
    }
}
```

## Schedules

Schedules run a set of Systems according to some execution strategy.
Systems can be added to any number of System Sets, which are used to control their scheduling metadata.

The built-in "parallel executor" considers dependencies between systems and (by default) run as many of
them in parallel as possible. This maximizes performance, while keeping the system execution safe. To control
the system ordering, define explicit dependencies between systems and their sets.


```rust
use voker_ecs::prelude::*;

#[derive(Component)]
struct Position { x: f32, y: f32 }
#[derive(Component)]
struct Velocity { x: f32, y: f32 }

// This system moves each entity with a Position and Velocity component
fn movement(mut query: Query<(&mut Position, &Velocity)>) {
    for (position, velocity) in query {
        position.x += velocity.x;
        position.y += velocity.y;
    }
}

fn main() {
    // Create a new empty World to hold our Entities and Components
    let mut world = World::alloc();

    // Spawn an entity with Position and Velocity components
    world.spawn((
        Position { x: 0.0, y: 0.0 },
        Velocity { x: 1.0, y: 0.0 },
    ));

    // Create a new Schedule, which defines an execution strategy for Systems
    let mut schedule = Schedule::default();

    // Add our system to the schedule
    schedule.add_system(movement);

    // Run the schedule once. If your app has a "loop", you would run this once per loop
    schedule.run(&mut world);
}
```

## Features

The following modules extend core ECS workflows for filtering, change tracking,
deferred mutation, message passing, and storage strategy control.

### Component Storage

voker ECS supports multiple component storage types.

* **Dense Table**: Fast and cache friendly iteration, but slower adding and removing of components. This is the default storage type.
* **Sparse Map**: Fast adding and removing of components, but slower iteration.

Component storage types are configurable, and they default to table storage if the storage is not manually defined.

```rust
use voker_ecs::prelude::*;

#[derive(Component)]
struct TableStoredComponent;

#[derive(Component)]
#[component(storage = "sparse")]
struct SparseStoredComponent;
```

See [`ComponentStorage`](crate::component::ComponentStorage) for more details.

### Component Bundles

Define sets of Components that should be added together.

```rust
use voker_ecs::prelude::*;

#[derive(Default, Component)]
struct Player;
#[derive(Default, Component)]
struct Position { x: f32, y: f32 }
#[derive(Default, Component)]
struct Velocity { x: f32, y: f32 }

#[derive(Bundle, Default)]
struct PlayerBundle {
    player: Player,
    position: Position,
    velocity: Velocity,
}

let mut world = World::alloc();

// Spawn a new entity and insert the default PlayerBundle
world.spawn(PlayerBundle::default());

// Bundles play well with Rust's struct update syntax
world.spawn(PlayerBundle {
    position: Position { x: 1.0, y: 1.0 },
    ..Default::default()
});
```

### Query Filters

```rust
use voker_ecs::prelude::*;

#[derive(Component)]
struct Position { x: f32, y: f32 }
#[derive(Component)]
struct Player;
#[derive(Component)]
struct Alive;

// Gets the Position component of all Entities with Player component
// and without the Alive component.
fn system(query: Query<&Position, And<(With<Player>, Without<Alive>)>>) {
    for position in query {
        // ...
    }
}
```

### Change Detection

voker ECS tracks _all_ changes to Components and Resources.

Queries can filter for changed Components:

```rust
use voker_ecs::prelude::*;

#[derive(Component)]
struct Position { x: f32, y: f32 }
#[derive(Component)]
struct Velocity { x: f32, y: f32 }

// Gets the Position component of all Entities whose Velocity has changed since the last run of the System
fn system_changed(query: Query<&Position, Changed<Velocity>>) {
    for position in query {
        // ..
    }
}

// Gets the Position component of all Entities that had a Velocity component added since the last run of the System
fn system_added(query: Query<&Position, Added<Velocity>>) {
    for position in query {
        // ..
    }
}
```

Resources also expose change state:

```rust
use voker_ecs::prelude::*;

#[derive(Resource)]
struct Time(f32);

// Prints "time changed!" if the Time resource has changed since the last run of the System
fn system(time: ResRef<Time>) {
    if time.is_changed() {
        println!("time changed!");
    }
}
```

### Deferred Commands

`Commands` is a deferred command queue interface that lets systems enqueue world mutations
without requiring immediate exclusive access to `World`.

```rust
use voker_ecs::prelude::*;

#[derive(Component)]
struct Disabled;

fn despawn_disabled(mut commands: Commands, query: Query<Entity, With<Disabled>>) {
    for entity in query {
        commands.despawn(entity);
    }
}
```

Commands are queued first, then applied later by `World::apply_commands()`
(typically by schedule execution flow).

### Messages

Messages are one-shot payloads sent between systems.
They are useful when you want to decouple producers and consumers without adding direct dependencies.

```rust
use voker_ecs::prelude::*;
use voker_ecs_derive::Message;

#[derive(Message)]
struct Collision {
    lhs: u32,
    rhs: u32,
}

fn detect_collisions(mut writer: MessageWriter<Collision>) {
    // Emit messages for this frame/update.
    // (Replace with real collision detection logic.)
    writer.write(Collision { lhs: 1, rhs: 2 });
}

fn handle_collisions(mut reader: MessageReader<Collision>) {
    for collision in reader.read() {
        // React to each unread collision message.
        let _ = (collision.lhs, collision.rhs);
    }
}
```

Message types should be registered in the world:

```rust
use voker_ecs::prelude::*;

#[derive(Message)]
struct Collision;

let mut world = World::alloc();
world.register_message::<Collision>();

// Run after a schedule pass to rotate message buffers globally.
world.update_messages();
```

Internally, each message resource uses a two-buffer lifecycle.
New writes go to the current buffer, and `update_messages` rotates buffers so readers
can still observe recent messages for one additional update.
