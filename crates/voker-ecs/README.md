
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
Worlds, much like `std::collections`'s `HashSet` and `Vec`,
expose operations to insert, read, write, and remove the data they store.

```rust
use voker_ecs::prelude::World;

let world = World::alloc();
```

## Components

Components are clonable normal Rust structs. They are data stored
in a `World` and specific instances of Components correlate to Entities.

```rust
use voker_ecs::prelude::*;

#[derive(Component, Clone)]
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

#[derive(Component, Clone)]
struct Position { x: f32, y: f32 }
#[derive(Component, Clone)]
struct Velocity { x: f32, y: f32 }

let mut world = World::alloc();

let entity: EntityOwned = world
    .spawn((Position { x: 0.0, y: 0.0 }, Velocity { x: 1.0, y: 0.0 }));

let position = entity.get::<Position>().unwrap();
let velocity = entity.get::<Velocity>().unwrap();
```

## Systems

Systems are normal Rust functions. Thanks to the Rust type system,
voker ECS can use function parameter types to determine what data
needs to be sent to the system. It also uses this "data access"
information to determine what Systems can run in parallel with each other.

```rust
use voker_ecs::prelude::*;

#[derive(Component, Clone)]
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
Systems can be added to any number of System Sets, which are used to
control their scheduling metadata.

The built-in "parallel executor" considers dependencies between systems
and (by default) run as many of them in parallel as possible. This maximizes
performance, while keeping the system execution safe. To control the system
ordering, define explicit dependencies between systems and their sets.

```rust
use voker_ecs::prelude::*;

#[derive(Component, Clone)]
struct Position { x: f32, y: f32 }

#[derive(Component, Clone)]
struct Velocity { x: f32, y: f32 }

// This system moves each entity with a Position and Velocity component
fn movement(mut query: Query<(&mut Position, &Velocity)>) {
    for (mut position, velocity) in query {
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

- **Dense Table**: Fast and cache friendly iteration, but slower adding and removing
  of components. This is the default storage type.
- **Sparse Map**: Fast adding and removing of components, but slower iteration.

Component storage types are configurable, and they default to table storage if the storage
is not manually defined.

```rust
use voker_ecs::prelude::*;

#[derive(Component, Clone)]
struct TableStoredComponent;

#[derive(Component, Clone)]
#[component(storage = "sparse")]
struct SparseStoredComponent;
```

See `StorageMode` for more details.

### Component Bundles

Define sets of Components that should be added together.

```rust
use voker_ecs::prelude::*;

#[derive(Default, Component, Clone)]
struct Player;

#[derive(Default, Component, Clone)]
struct Position { x: f32, y: f32 }

#[derive(Default, Component, Clone)]
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

Query filters let you narrow down which entities are matched without changing
the queried data type itself. Filters can be combined with `And` and `Or`, and
can also be mixed with change-tracking filters like `Changed<T>` and `Added<T>`.



```rust
use voker_ecs::prelude::*;

#[derive(Component, Clone)]
struct Position { x: f32, y: f32 }
#[derive(Component, Clone)]
struct Player;
#[derive(Component, Clone)]
struct Alive;

// Gets the Position component of all Entities with Player component
// and without the Alive component.
fn system(query: Query<&Position, With<Player>>) {
    for position in query {
        // ...
    }
}
```

You can also express more complex selection logic:

```rust
use voker_ecs::prelude::*;

#[derive(Component, Clone)]
struct Position { x: f32, y: f32 }
#[derive(Component, Clone)]
struct Velocity { x: f32, y: f32 }
#[derive(Component, Clone)]
struct Player;
#[derive(Component, Clone)]
struct Enemy;
#[derive(Component, Clone)]
struct Disabled;

// Select moving players or enemies, skip disabled entities,
// and only include ones whose Velocity changed.
fn selective_movement_targets(
    query: Query<
        (&Position, &Velocity),
        And<(
            Or<(With<Player>, With<Enemy>)>,
            Without<Disabled>,
            Changed<Velocity>,
        )>,
    >,
) {
    for (position, velocity) in query {
        // ...
    }
}
```

### Change Detection

voker ECS tracks _all_ changes to Components and Resources.

Queries can filter for changed Components:

```rust
use voker_ecs::prelude::*;

#[derive(Component, Clone)]
struct Position { x: f32, y: f32 }
#[derive(Component, Clone)]
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

#[derive(Component, Clone)]
struct Disabled;

fn despawn_disabled(mut commands: Commands, query: Query<Entity, With<Disabled>>) {
    for entity in query {
        commands.despawn(entity);
    }
}
```

Commands are queued first, then applied later by `World::flush()`
(typically by schedule execution flow).

### Messages

Messages are one-shot payloads sent between systems. They are useful
when you want to decouple producers and consumers without adding direct dependencies.

```rust
use voker_ecs::prelude::*;

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

### Observers

Observers are systems that watch for a "trigger" of a specific `Event`:

```rust
use voker_ecs::prelude::*;

#[derive(Event)]
struct Speak {
    message: String
}

let mut world = World::alloc();

world.add_observer(|event: On<Speak>| {
    println!("{}", event.message);
});

world.trigger(Speak {
    message: "Hello!".to_string(),
});
```

These differ from `MessageReader` and `MessageWriter` in that they are "reactive".
Rather than happening at a specific point in a schedule, they happen _immediately_ whenever a trigger happens.
Triggers can trigger other triggers, and they all will be evaluated at the same time!

If the event is an `EntityEvent`, it can also be triggered to target specific entities:

```rust
use voker_ecs::prelude::*;

#[derive(EntityEvent)]
struct Explode {
    #[event_target]
    entity: Entity,
}

let mut world = World::alloc();
let entity = world.spawn_empty().entity();

world.add_observer(|explode: On<Explode>, mut commands: Commands| {
    println!("Entity {} goes BOOM!", explode.entity);
    commands.with_entity(explode.entity).despawn();
});

world.trigger(Explode { entity });
```
