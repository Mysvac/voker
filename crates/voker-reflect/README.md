# Runtime reflection system

This library implements a dynamic reflection system in Rust, designed to provide
comprehensive runtime type information and data manipulation capabilities.

While it's a general-purpose reflection system suitable for various scenarios,
it's specifically designed for the voker Engine and may include
platform-specific dependencies from voker that could be redundant in
non-game-engine contexts.

## Goals

As a dynamic reflection system, this library aims to support:

- **Runtime Type Information**:
    - Basic information: type names, TypeId, field lists, generic parameters
    - Custom attributes: similar to C# attributes, allowing user-defined metadata on types
    - Type documentation (optional): useful for game engine editors and tools
    - See more information in [`voker_reflect::info`].

- **Data Manipulation**:
    - Type erasure: achieve effects similar to `Object` in other languages through trait objects
    - Specialized interfaces through reflection subtraits: `Struct`, `Enum`, etc.
    - Dynamic object composition with ability to apply to concrete types when needed
    - See more information in [`voker_reflect::ops`] and [`voker_reflect::Reflect`].

- **Type Registration**:
    - Metadata: type metadata containing both type information and available function pointers
    - Registry: storage system for metadata enabling type information retrieval without instances
    - Auto-registration: type registration through static initialization
    - See more information in [`voker_reflect::registry`].

- **Trait Reflection**:
    - Trait reflection based on registration system, enabling dynamic trait object retrieval
    - See more information in [`registry::TypeData`] and [`derive::reflect_trait`]

- **Reflection Macros**:
    - Automatic generation of reflection implementations for types
    - See more information in [`voker_reflect::derive`].

- **(De)Serialization**:
    - (De)Serialization system based on registry, allowing types without explicit `Serialize`/`Deserialize` implementations
    - See more information in [`voker_reflect::serde`].

- **Path-Based Access**:
    - Multi-level data access via string paths (struct fields, array elements, etc.)
    - See more information in [`voker_reflect::access`].

## Examples


### Derive reflection and inspect type info

```rust
use voker_reflect::prelude::*;

#[derive(Reflect)]
struct Player {
    name: String,
    level: u32,
    health: f32,
}

let info = <Player as Typed>::type_info().as_struct().unwrap();

assert_eq!(info.field_len(), 3);
assert_eq!(info.index_of("name"), Some(0));
assert_eq!(info.index_of("health"), Some(2));
```

### Read nested data with path access

```rust
use voker_reflect::prelude::*;

#[derive(Reflect)]
struct Inventory {
    coins: u32,
    slots: Vec<Option<&'static str>>,
}

let inventory = Inventory {
    coins: 1234,
    slots: vec![Some("Sword"), None],
};

// One-off access
assert_eq!(inventory.access_as::<u32>(".coins"), Ok(&1234_u32));

// Reuseable accessor
let accessor = PathAccessor::parse_static(".slots[0]").unwrap();
let first_slot = *accessor.access_as::<Option<&str>>(&inventory).unwrap();
assert_eq!(first_slot, Some("Sword"));
```

### Register a type and operate it dynamically

```rust
use core::any::TypeId;
use voker_reflect::prelude::*;
use voker_reflect::ops::Struct;

#[derive(Reflect, Default)]
#[reflect(Default)]
struct Enemy {
    species: String,
    hp: u32,
}

let type_id = TypeId::of::<Enemy>();

// Dynamic operation
let mut registry = TypeRegistry::default();
registry.auto_register();

assert!(registry.contains(type_id));

let default_ctor = registry.get_type_data::<ReflectDefault>(type_id).unwrap();
let mut dyn_obj: Box<dyn Reflect> = default_ctor.default();
let dyn_struct: &mut dyn Struct = dyn_obj.reflect_mut().as_struct().unwrap();
dyn_struct.field_mut("hp").unwrap().apply(&100u32);

// Downcast
let enemy: Enemy = dyn_obj.take::<Enemy>().unwrap();
assert!(enemy.species.is_empty());
assert_eq!(enemy.hp, 100);
```

### Reflect based serialization and deserialization

```rust
use serde_core::de::DeserializeSeed;
use core::marker::PhantomData;
use voker_reflect::prelude::*;

#[derive(Reflect, PartialEq, Debug)]
#[type_path = "example::MyStruct"]
struct MyStruct {
    value: i32,
    #[reflect(skip_serde)]
    _marker: PhantomData<i32>,
    // or use `#[reflect(ignore, clone, default)]`
    // for non-reflective.
}

let mut registry = TypeRegistry::new();
registry.register::<MyStruct>();

let value = MyStruct { value: 123, _marker: PhantomData };

// serialize
let driver = ReflectSerializeDriver::new(&value, &registry);
let text = ron::to_string(&driver).unwrap();
assert_eq!(text, r#"{"example::MyStruct":(value:123)}"#);

// deserialize
let mut wrapper = ron::Deserializer::from_str(&text).unwrap();
let driver = ReflectDeserializeDriver::new(&registry);
let dynamic: Box<dyn Reflect> = driver.deserialize(&mut wrapper).unwrap();

// If ReflectFromReflect conversion is available and succeeds, this is concrete.
// Otherwise `dynamic` remains a dynamic reflected value.
assert_eq!(dynamic.take::<MyStruct>().unwrap(), value);
```

## Feature Flags

### `default`

Includes `std` , `debug` and `auto_register`.

### `std`

Enabled by default.

Provide reflection implementations for standard library containers like `HashMap`.

### `debug`

Enabled by default, when turned on, we will record type information
stack during serialization and deserialization.

### `reflect_docs`

Enables type documentation collection. Automatically gathers standard documentation
from `#[doc = "..."]` attributes. Disabled by default.

When disabled, documentation functions remain available but always return empty values.

See [`TypeInfo::docs`](crate::info::TypeInfo::docs) for details.

[`Struct`]: ops::Struct
[`Enum`]: ops::Enum
[`Tuple`]: ops::Tuple