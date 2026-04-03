use alloc::format;

use serde_core::ser::{self, SerializeMap};
use serde_core::{Serialize, Serializer};

use super::SerializeProcessor;
use super::array_serializer::ArraySerializer;
use super::enum_serializer::EnumSerializer;
use super::list_serializer::ListSerializer;
use super::map_serializer::MapSerializer;
use super::set_serializer::SetSerializer;
use super::struct_serializer::StructSerializer;
use super::tuple_serializer::TupleSerializer;
use super::tuple_struct_serializer::TupleStructSerializer;

crate::cfg::debug! {
    use super::error_utils::TYPE_INFO_STACK;
}

use crate::Reflect;
use crate::ops::ReflectRef;
use crate::registry::{ReflectSerialize, TypeRegistry};

// -----------------------------------------------------------------------------
// SerializeDriver

/// Serializer for reflected types excluding type path information.
///
/// # Serialization Rules
///
/// The serializer follows a three-step priority order:
///
/// 1. **Processor Priority**: First attempts to use the provided [`SerializeProcessor`].
///    If the processor handles the type (successfully or with an error), its result is returned immediately.
///
/// 2. **Trait Fallback**: If no processor is available, looks for [`ReflectSerialize`]
///    in the type metadata and uses its implementation.
///
/// 3. **Reflection Default**: As a last resort, uses the reflection system's default serialization method.
///
/// For custom `Opaque` types, the reflection system does **not** provide default serialization.
/// Users must annotate these types with `#[reflect(serialize)]` to supply a serde-based `Serialize` implementation.
///
/// # Type Path Context
///
/// This serializer is designed for **type-erased serialization** scenarios.
///
/// For example, consider a type `Foo`:
///
/// ```text
/// struct Foo {
///     id: u32,
///     name: String,
/// }
/// ```
///
/// The serialized result (in JSON syntax) would be:
///
/// ```text
/// {
///   "id": 0,
///   "name": "..."
/// }
/// ```
///
/// While this resembles regular serialization, type erasure in the reflection system introduces a limitation:
/// such data contains no type information, making deserialization impossible without additional context.
///
/// Therefore, the corresponding [`DeserializeDriver`] requires a [`TypeMeta`] parameter to specify the target type.
///
/// In contrast, [`ReflectSerializeDriver`] retains the outermost type name during serialization
/// while internally using [`SerializeDriver`]:
///
/// ```text
/// {
///   "module_path::Foo": {
///     "id": 0,
///     "name": "..."
///   }
/// }
/// ```
///
/// This corresponds to [`ReflectDeserializeDriver`], which can query type information directly from the registry
/// using the embedded type name, eliminating the need for an explicit [`TypeMeta`] parameter.
///
/// For more information, see [`ReflectSerializeDriver`], [`DeserializeDriver`], and [`ReflectDeserializeDriver`].
///
/// # Examples
///
/// ```
/// # use voker_reflect::{registry::TypeRegistry, serde::SerializeDriver, Reflect};
/// #
/// #[derive(Reflect, PartialEq, Debug)]
/// struct MyStruct {
///   value: i32
/// }
///
/// let mut registry = TypeRegistry::new();
/// registry.register::<MyStruct>();
///
/// let input = MyStruct { value: 123 };
///
/// let serializer = SerializeDriver::new(&input, &registry);
///
/// let output = ron::to_string(&serializer).unwrap();
///
/// assert_eq!(output, r#"(value:123)"#);
/// ```
///
/// [`DeserializeDriver`]: crate::serde::DeserializeDriver
/// [`ReflectDeserializeDriver`]: crate::serde::ReflectDeserializeDriver
/// [`TypeMeta`]: crate::registry::TypeMeta
pub struct SerializeDriver<'a, P: SerializeProcessor = ()> {
    value: &'a dyn Reflect,
    registry: &'a TypeRegistry,
    processor: Option<&'a P>,
}

impl<'a> SerializeDriver<'a, ()> {
    /// Creates a serializer with no processor.
    ///
    /// If you want to add custom logic for serializing certain values, use
    /// [`with_processor`](Self::with_processor).
    #[inline]
    pub const fn new(value: &'a dyn Reflect, registry: &'a TypeRegistry) -> Self {
        Self {
            value,
            registry,
            processor: None,
        }
    }
}

impl<'a, P: SerializeProcessor> SerializeDriver<'a, P> {
    /// Creates a serializer with a processor.
    #[inline]
    pub const fn with_processor(
        value: &'a dyn Reflect,
        registry: &'a TypeRegistry,
        processor: &'a P,
    ) -> Self {
        Self {
            value,
            registry,
            processor: Some(processor),
        }
    }

    #[inline]
    pub(super) const fn new_internal(
        value: &'a dyn Reflect,
        registry: &'a TypeRegistry,
        processor: Option<&'a P>,
    ) -> Self {
        Self {
            value,
            registry,
            processor,
        }
    }
}

impl<'a, P: SerializeProcessor> Serialize for SerializeDriver<'a, P> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let serializer = if let Some(processor) = self.processor {
            match processor.try_serialize(self.value, self.registry, serializer) {
                Ok(result) => return result,
                Err(serializer) => serializer, // Processor did not handle this value.
            }
        } else {
            serializer
        };

        // Prefer the type's registered serde serializer when available.
        if let Some(p) = self.registry.get_type_data::<ReflectSerialize>(self.value.type_id()) {
            return p.serialize(self.value, serializer);
        }

        crate::cfg::debug! {
            if let Some(info) = self.value.represented_type_info() {
                TYPE_INFO_STACK.with_borrow_mut(|stack|stack.push(info));
            } else {
                TYPE_INFO_STACK.with_borrow_mut(|stack|stack.push(self.value.reflect_type_info()));
            }
        }

        let output: Result<S::Ok, S::Error> = match self.value.reflect_ref() {
            ReflectRef::Struct(struct_value) => StructSerializer {
                struct_value,
                registry: self.registry,
                processor: self.processor,
            }
            .serialize(serializer),
            ReflectRef::TupleStruct(tuple_struct) => TupleStructSerializer {
                tuple_struct,
                registry: self.registry,
                processor: self.processor,
            }
            .serialize(serializer),
            ReflectRef::Tuple(tuple) => TupleSerializer {
                tuple,
                registry: self.registry,
                processor: self.processor,
            }
            .serialize(serializer),
            ReflectRef::List(list) => ListSerializer {
                list,
                registry: self.registry,
                processor: self.processor,
            }
            .serialize(serializer),
            ReflectRef::Array(array) => ArraySerializer {
                array,
                registry: self.registry,
                processor: self.processor,
            }
            .serialize(serializer),
            ReflectRef::Map(map) => MapSerializer {
                map,
                registry: self.registry,
                processor: self.processor,
            }
            .serialize(serializer),
            ReflectRef::Set(set) => SetSerializer {
                set,
                registry: self.registry,
                processor: self.processor,
            }
            .serialize(serializer),
            ReflectRef::Enum(enum_value) => EnumSerializer {
                enum_value,
                registry: self.registry,
                processor: self.processor,
            }
            .serialize(serializer),
            ReflectRef::Opaque(_) => Err(ser::Error::custom(format!(
                "No default serialization method is available for this opaque type: `{}`. Register ReflectSerialize (for example via #[reflect(serialize)]).",
                self.value.reflect_type_path(),
            ))),
        };

        crate::cfg::debug! {
            TYPE_INFO_STACK.with_borrow_mut(|stack|stack.pop());
        }

        output
    }
}

// -----------------------------------------------------------------------------
// ReflectSerializeDriver

/// General-purpose serializer for reflected types with type path information.
///
/// For serialization without type path wrapping, see [`SerializeDriver`].
///
/// # Serialization Rules
///
/// The serializer follows a three-step priority order:
///
/// 1. **Processor Priority**: First attempts to use the provided [`SerializeProcessor`].
///    If the processor handles the type (successfully or with an error), its result is returned immediately.
///
/// 2. **Trait Fallback**: If no processor is available, looks for [`ReflectSerialize`]
///    in the type registry and uses its implementation.
///
/// 3. **Reflection Default**: As a last resort, uses the reflection system's default serialization method.
///
/// For custom `Opaque` types, the reflection system does **not** provide default serialization.
/// Users must annotate these types with `#[reflect(serialize)]` to supply a serde-based `Serialize` implementation.
///
/// # Output Format
///
/// This serializer outputs a map containing a single entry:
/// - **Key**: The _full_ type path of the reflected type
/// - **Value**: The serialized data of the type
///
/// Example (JSON representation):
/// ```json
/// {
///   "foo::utils::Foo": {
///     "field1": "value1",
///     "field2": 42
///   }
/// }
/// ```
///
/// This only retains the outermost type path and still uses [`SerializeDriver`] internally.
///
/// # Examples
///
/// ```
/// # use voker_reflect::prelude::{TypeRegistry, ReflectSerializeDriver, Reflect};
/// #
/// #[derive(Reflect)]
/// #[reflect(type_path = "my_crate::MyStruct")]
/// struct MyStruct {
///   value: i32
/// }
///
/// let mut registry = TypeRegistry::new();
/// registry.auto_register();
///
/// let input = MyStruct { value: 123 };
///
/// let serializer = ReflectSerializeDriver::new(&input, &registry);
/// let output = ron::to_string(&serializer).unwrap();
///
/// assert_eq!(output, r#"{"my_crate::MyStruct":(value:123)}"#);
/// ```
pub struct ReflectSerializeDriver<'a, P: SerializeProcessor = ()> {
    value: &'a dyn Reflect,
    registry: &'a TypeRegistry,
    processor: Option<&'a P>,
}

impl<'a> ReflectSerializeDriver<'a, ()> {
    /// Creates a serializer with no processor.
    ///
    /// If you want to add custom logic for serializing certain values, use
    /// [`with_processor`](Self::with_processor).
    #[inline]
    pub fn new(value: &'a dyn Reflect, registry: &'a TypeRegistry) -> Self {
        Self {
            value,
            registry,
            processor: None,
        }
    }
}

impl<'a, P: SerializeProcessor> ReflectSerializeDriver<'a, P> {
    /// Creates a serializer with a processor.
    ///
    /// If you do not need any custom logic for handling certain values, use
    /// [`new`](Self::new).
    #[inline]
    pub fn with_processor(
        value: &'a dyn Reflect,
        registry: &'a TypeRegistry,
        processor: &'a P,
    ) -> Self {
        Self {
            value,
            registry,
            processor: Some(processor),
        }
    }
}

impl<P: SerializeProcessor> Serialize for ReflectSerializeDriver<'_, P> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        crate::cfg::debug! {
            // Perhaps useless, it can be cleared by `pop` usually.
            TYPE_INFO_STACK.with_borrow_mut(|stack|stack.clear());
        }

        let Some(info) = self.value.represented_type_info() else {
            return Err(ser::Error::custom(format!(
                "cannot get represented type from type: `{}`.",
                self.value.reflect_type_path(),
            )));
        };

        let mut state = serializer.serialize_map(Some(1))?;
        state.serialize_entry(
            info.type_path(),
            &SerializeDriver::new_internal(self.value, self.registry, self.processor),
        )?;

        state.end()
    }
}
