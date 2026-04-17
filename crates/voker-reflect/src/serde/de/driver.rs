use alloc::boxed::Box;
use alloc::format;
use core::fmt;

use serde_core::Deserializer;
use serde_core::de::{DeserializeSeed, Error, IgnoredAny, MapAccess, Visitor};

use super::DeserializeProcessor;
use super::array_visitor::ArrayVisitor;
use super::enum_visitor::EnumVisitor;
use super::list_visitor::ListVisitor;
use super::map_visitor::MapVisitor;
use super::option_visitor::OptionVisitor;
use super::set_visitor::SetVisitor;
use super::struct_visitor::StructVisitor;
use super::tuple_struct_visitor::TupleStructVisitor;
use super::tuple_visitor::TupleVisitor;

use crate::Reflect;
use crate::info::{TypeInfo, Typed};
use crate::registry::{GetTypeMeta, TypeMeta, TypeRegistry};
use crate::registry::{ReflectDeserialize, ReflectFromReflect};

crate::cfg::backtrace! {
    use super::error_utils::TYPE_INFO_STACK;
}

// -----------------------------------------------------------------------------
// DeserializeDriver

/// Deserializer for reflected types excluding type path information.
///
/// The target type is specified via the [`TypeMeta`] parameter.
///
/// # Deserialization Rules
///
/// The deserializer follows this three-step priority order:
///
/// 1. **Processor First**: Attempt to use the [`DeserializeProcessor`] if provided.
///    If the processor supports the type (successfully or with an error), return its result immediately.
///
/// 2. **Type Trait Fallback**: If no processor is available, look for [`ReflectDeserialize`]
///    in the [`TypeMeta`] and use its implementation.
///
/// 3. **Reflection Default**: As a last resort, use the reflection system's default deserialization method,
///    which returns dynamic types always.
///
/// For custom `Opaque` types, the reflection system does **not** provide a default deserialization implementation.
/// Users must annotate the type with `#[reflect(Deserialize)]` to supply a serde-based `Deserialize` implementation.
///
/// ## Why Default Method Returns Dynamic Type
///
/// [`ReflectFromReflect`] copies data rather than moving it.
/// If automatic type conversion were attempted after each parsing step,
/// it would result in copying data at every level (exponential cost based on type nesting depth).
///
/// Therefore, type conversion is performed only once during the final step of [`ReflectDeserializeDriver`],
/// while [`DeserializeDriver`] returns dynamic types for intermediate processing.
///
/// # Type Path Context
///
/// Unlike [`ReflectDeserializeDriver`], which expects complete type path for root objects,
/// `DeserializeDriver` consumes only the internal data structure, with the target type determined by
/// the `TypeMeta` parameter.
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
/// The deserialization input (in JSON syntax) would be:
///
/// ```text
/// {
///   "id": 0,
///   "name": "..."
/// }
/// ```
///
/// Due to the lack of type information in the text itself, [`DeserializeDriver`] needs to carry [`TypeMeta`] data.
///
/// In contrast, [`ReflectDeserializeDriver`] need the outermost type path during deserialization,
/// while internally using [`DeserializeDriver`]:
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
/// It can query type information directly from the registry using the embedded type path,
/// eliminating the need for an explicit [`TypeMeta`] parameter.
///
/// For more information, see [`ReflectDeserializeDriver`], [`SerializeDriver`], and [`ReflectSerializeDriver`].
///
/// # Examples
///
/// ```
/// # use core::any::TypeId;
/// # use serde_core::de::DeserializeSeed;
/// # use voker_reflect::{Reflect, FromReflect, serde::DeserializeDriver};
/// # use voker_reflect::{ops::DynamicStruct, registry::{TypeRegistry, ReflectFromReflect}};
/// #[derive(Reflect, PartialEq, Debug)]
/// struct MyStruct {
///   value: i32
/// }
///
/// let mut registry = TypeRegistry::default();
/// registry.register::<MyStruct>();
///
/// let input = r#"(
///   value: 123
/// )"#;
///
/// let meta = registry.get(TypeId::of::<MyStruct>()).unwrap();
///
/// let mut data = ron::Deserializer::from_str(input).unwrap();
/// let deserializer = DeserializeDriver::new(meta, &registry);
///
/// let output: Box<dyn Reflect> = deserializer.deserialize(&mut data).unwrap();
///
/// // Since `MyStruct` is not an opaque type and does not register `ReflectDeserialize`,
/// // we know that its deserialized value will be a `DynamicStruct`,
/// // although it will represent `MyStruct`.
/// assert!(output.represents::<MyStruct>());
///
/// // We can convert back to `MyStruct` using `FromReflect`.
/// let value: MyStruct = <MyStruct as FromReflect>::from_reflect(&*output).unwrap();
/// assert_eq!(value, MyStruct { value: 123 });
///
/// // We can also do this dynamically with `ReflectFromReflect`.
/// let type_id = output.represented_type_info().unwrap().type_id();
/// let from_reflect = registry.get_type_data::<ReflectFromReflect>(type_id).unwrap();
/// let value: Box<dyn Reflect> = from_reflect.from_reflect(&*output).unwrap();
/// assert!(value.is::<MyStruct>());
/// assert_eq!(value.take::<MyStruct>().unwrap(), MyStruct { value: 123 });
/// ```
///
/// [`SerializeDriver`]: crate::serde::SerializeDriver
/// [`ReflectSerializeDriver`]: crate::serde::ReflectSerializeDriver
pub struct DeserializeDriver<'a, P: DeserializeProcessor = ()> {
    type_meta: &'a TypeMeta,
    registry: &'a TypeRegistry,
    processor: Option<&'a mut P>,
}

impl<'a> DeserializeDriver<'a, ()> {
    /// Creates a typed deserializer with no processor.
    ///
    /// If you want to add custom logic for deserializing certain types, use
    /// [`with_processor`](Self::with_processor).
    #[inline]
    pub fn new(type_meta: &'a TypeMeta, registry: &'a TypeRegistry) -> Self {
        Self {
            type_meta,
            registry,
            processor: None,
        }
    }

    /// Creates a new [`DeserializeDriver`] for the given type `T`
    /// without a processor.
    ///
    /// # Panics
    ///
    /// Panics if `T` is not registered in the given [`TypeRegistry`].
    #[inline]
    pub fn of<T: Typed + GetTypeMeta>(registry: &'a TypeRegistry) -> Self {
        let type_meta = registry
            .get(core::any::TypeId::of::<T>())
            .unwrap_or_else(|| panic!("no TypeMeta found for type `{}`", T::type_path()));

        Self {
            type_meta,
            registry,
            processor: None,
        }
    }
}

impl<'a, P: DeserializeProcessor> DeserializeDriver<'a, P> {
    /// Creates a typed deserializer with a processor.
    ///
    /// If you do not need any custom logic for handling certain types, use
    /// [`new`](Self::new).
    #[inline]
    pub fn with_processor(
        type_meta: &'a TypeMeta,
        registry: &'a TypeRegistry,
        processor: &'a mut P,
    ) -> Self {
        Self {
            type_meta,
            registry,
            processor: Some(processor),
        }
    }

    /// An internal constructor for creating a deserializer without resetting the type info stack.
    #[inline]
    pub(super) fn new_internal(
        type_meta: &'a TypeMeta,
        registry: &'a TypeRegistry,
        processor: Option<&'a mut P>,
    ) -> Self {
        Self {
            type_meta,
            registry,
            processor,
        }
    }
}

impl<'de, P: DeserializeProcessor> DeserializeSeed<'de> for DeserializeDriver<'_, P> {
    type Value = Box<dyn Reflect>;

    fn deserialize<D: Deserializer<'de>>(
        mut self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error> {
        let deserializer = if let Some(processor) = self.processor.as_deref_mut() {
            match processor.try_deserialize(self.type_meta, self.registry, deserializer) {
                Ok(Ok(value)) => return Ok(value),
                Ok(Err(err)) => return Err(err),
                Err(deserializer) => deserializer,
            }
        } else {
            deserializer
        };

        if let Some(deserialize_reflect) = self.type_meta.get_data::<ReflectDeserialize>() {
            return deserialize_reflect.deserialize(deserializer);
        }

        crate::cfg::backtrace! {
            TYPE_INFO_STACK.with_borrow_mut(|stack|stack.push(self.type_meta.type_info()))
        }

        let dynamic_value: Result<Box<dyn Reflect>, D::Error> = match self.type_meta.type_info() {
            TypeInfo::Struct(struct_info) => {
                let mut dynamic_struct = deserializer.deserialize_struct(
                    struct_info.type_ident(),
                    struct_info.field_names(),
                    StructVisitor {
                        struct_info,
                        registry: self.registry,
                        processor: self.processor,
                    },
                )?;
                dynamic_struct.set_type_info(Some(self.type_meta.type_info()));
                Ok(Box::new(dynamic_struct))
            }
            TypeInfo::TupleStruct(tuple_struct_info) => {
                let is_new_type = tuple_struct_info.field_len() == 1
                    && !tuple_struct_info.field(0).unwrap().skip_serde();

                let mut dynamic_tuple_struct = if is_new_type {
                    deserializer.deserialize_newtype_struct(
                        tuple_struct_info.type_ident(),
                        TupleStructVisitor {
                            tuple_struct_info,
                            registry: self.registry,
                            processor: self.processor,
                        },
                    )?
                } else {
                    deserializer.deserialize_tuple_struct(
                        tuple_struct_info.type_ident(),
                        tuple_struct_info.field_len(),
                        TupleStructVisitor {
                            tuple_struct_info,
                            registry: self.registry,
                            processor: self.processor,
                        },
                    )?
                };
                dynamic_tuple_struct.set_type_info(Some(self.type_meta.type_info()));
                Ok(Box::new(dynamic_tuple_struct))
            }
            TypeInfo::Tuple(tuple_info) => {
                let mut dynamic_tuple = deserializer.deserialize_tuple(
                    tuple_info.field_len(),
                    TupleVisitor {
                        tuple_info,
                        registry: self.registry,
                        processor: self.processor,
                    },
                )?;
                dynamic_tuple.set_type_info(Some(self.type_meta.type_info()));
                Ok(Box::new(dynamic_tuple))
            }
            TypeInfo::List(list_info) => {
                let mut dynamic_list = deserializer.deserialize_seq(ListVisitor {
                    list_info,
                    registry: self.registry,
                    processor: self.processor,
                })?;
                dynamic_list.set_type_info(Some(self.type_meta.type_info()));
                Ok(Box::new(dynamic_list))
            }
            TypeInfo::Array(array_info) => {
                let mut dynamic_array = deserializer.deserialize_tuple(
                    array_info.len(),
                    ArrayVisitor {
                        array_info,
                        registry: self.registry,
                        processor: self.processor,
                    },
                )?;
                dynamic_array.set_type_info(Some(self.type_meta.type_info()));
                Ok(Box::new(dynamic_array))
            }
            TypeInfo::Map(map_info) => {
                let mut dynamic_map = deserializer.deserialize_map(MapVisitor {
                    map_info,
                    registry: self.registry,
                    processor: self.processor,
                })?;
                dynamic_map.set_type_info(Some(self.type_meta.type_info()));
                Ok(Box::new(dynamic_map))
            }
            TypeInfo::Set(set_info) => {
                let mut dynamic_set = deserializer.deserialize_seq(SetVisitor {
                    set_info,
                    registry: self.registry,
                    processor: self.processor,
                })?;
                dynamic_set.set_type_info(Some(self.type_meta.type_info()));
                Ok(Box::new(dynamic_set))
            }
            TypeInfo::Enum(enum_info) => {
                let mut dynamic_enum = if enum_info.type_ident() == "Option"
                    && enum_info.module_path() == Some("core::option")
                {
                    deserializer.deserialize_option(OptionVisitor {
                        enum_info,
                        registry: self.registry,
                        processor: self.processor,
                    })?
                } else {
                    deserializer.deserialize_enum(
                        enum_info.type_ident(),
                        enum_info.variant_names(),
                        EnumVisitor {
                            enum_info,
                            registry: self.registry,
                            processor: self.processor,
                        },
                    )?
                };
                dynamic_enum.set_type_info(Some(self.type_meta.type_info()));
                Ok(Box::new(dynamic_enum))
            }
            TypeInfo::Opaque(_) => Err(Error::custom(
                "No default deserialization method is available for this opaque type. Register ReflectDeserialize (for example via #[reflect(Deserialize)]).",
            )),
        };

        crate::cfg::backtrace! {
            TYPE_INFO_STACK.with_borrow_mut(|stack|stack.pop())
        }

        dynamic_value
    }
}

// -----------------------------------------------------------------------------
// ReflectDeserializeDriver

/// A general purpose deserializer for reflected types.
///
/// This is the deserializer counterpart to [`ReflectSerializeDriver`].
///
/// See [`DeserializeDriver`] for a deserializer that expects a known type.
///
/// # Deserialization Rules
///
/// The deserializer follows this three-step priority order:
///
/// 1. **Processor First**: Attempt to use the [`DeserializeProcessor`] if provided.
///    If the processor supports the type (successfully or with an error), return its result immediately.
///
/// 2. **Type Trait Fallback**: If no processor is available, look for [`ReflectDeserialize`]
///    in the [`TypeMeta`] and use its implementation.
///
/// 3. **Reflection Default**: As a last resort, use the reflection system's default deserialization method.
///    Finally, try using the [`ReflectFromReflect`] conversion type.
///
/// For custom `Opaque` types, the reflection system does **not** provide a default deserialization implementation.
/// Users must annotate the type with `#[reflect(Deserialize)]` to supply a serde-based `Deserialize` implementation.
///
/// # Input
///
/// This deserializer expects a map with a single entry,
/// where the key is the _full_ [type path] of the reflected type
/// and the value is the serialized data.
///
/// For examples:
///
/// ```json
/// {
///   "foo::utils::Foo": {
///     "field1": "value1",
///     "field2": 42
///   }
/// }
/// ```
///
/// # Output
///
/// This deserializer will return a [`Box<dyn Reflect>`] containing the deserialized data.
///
/// For the types that registered [`ReflectDeserialize`] type trait, this `Box` will contain the expected type
/// **if feasible**. For example, deserializing an `i32` will return a `Box<i32>` (as a `Box<dyn Reflect>`).
///
/// Otherwise, this `Box` will contain the dynamic equivalent.
/// For example, a deserialized struct might return a [`Box<DynamicStruct>`].
///
/// This means that if the actual type is needed, these dynamic representations will need to
/// be converted to the concrete type using [`FromReflect`] or [`ReflectFromReflect`] manually.
///
/// # Example
///
/// ```
/// # use serde_core::de::DeserializeSeed;
/// # use voker_reflect::prelude::{Reflect, FromReflect, TypeRegistry, ReflectDeserializeDriver};
/// # use voker_reflect::ops::DynamicStruct;
/// #
/// #[derive(Reflect, PartialEq, Debug)]
/// #[reflect(type_path = "my_crate::MyStruct")]
/// struct MyStruct {
///   value: i32
/// }
///
/// let mut registry = TypeRegistry::new();
/// registry.register::<MyStruct>();
///
/// let input = r#"{
///   "my_crate::MyStruct": (
///     value: 123
///   )
/// }"#;
///
/// let mut data = ron::Deserializer::from_str(input).unwrap();
/// let deserializer = ReflectDeserializeDriver::new(&registry);
///
/// let output: Box<dyn Reflect> = deserializer.deserialize(&mut data).unwrap();
///
/// // The driver performs one final conversion attempt via `ReflectFromReflect`.
/// // When that trait exists and conversion succeeds, this is a concrete value.
/// // Otherwise the dynamic value is returned unchanged.
/// assert!(output.is::<MyStruct>());
///
/// assert_eq!(output.take::<MyStruct>().unwrap(), MyStruct { value: 123 });
/// ```
///
/// [`ReflectSerializeDriver`]: crate::serde::ReflectSerializeDriver
/// [`Box<dyn Reflect>`]: crate::Reflect
/// [`Box<DynamicStruct>`]: crate::ops::DynamicStruct
/// [`Box<DynamicList>`]: crate::ops::DynamicList
/// [`FromReflect`]: crate::FromReflect
pub struct ReflectDeserializeDriver<'a, P: DeserializeProcessor = ()> {
    registry: &'a TypeRegistry,
    processor: Option<&'a mut P>,
}

impl<'a> ReflectDeserializeDriver<'a, ()> {
    #[inline]
    pub fn new(registry: &'a TypeRegistry) -> Self {
        Self {
            registry,
            processor: None,
        }
    }
}

impl<'a, P: DeserializeProcessor> ReflectDeserializeDriver<'a, P> {
    #[inline]
    pub fn with_processor(registry: &'a TypeRegistry, processor: &'a mut P) -> Self {
        Self {
            registry,
            processor: Some(processor),
        }
    }
}

impl<'de, P: DeserializeProcessor> DeserializeSeed<'de> for ReflectDeserializeDriver<'_, P> {
    type Value = Box<dyn Reflect>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        struct ReflectDeserializeDriverVisitor<'a, P> {
            registry: &'a TypeRegistry,
            processor: Option<&'a mut P>,
        }

        impl<'de, P: DeserializeProcessor> Visitor<'de> for ReflectDeserializeDriverVisitor<'_, P> {
            type Value = Box<dyn Reflect>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter
                    .write_str("map containing `type` and `value` entries for the reflected value")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                // Resolve the target type metadata from the registry.
                let type_meta = map
                    .next_key_seed(TypePathDeserializer::new(self.registry))?
                    .ok_or_else(|| Error::invalid_length(0, &"a single entry"))?;

                let value = map.next_value_seed(DeserializeDriver::new_internal(
                    type_meta,
                    self.registry,
                    self.processor,
                ))?;

                if map.next_key::<IgnoredAny>()?.is_some() {
                    return Err(Error::invalid_length(2, &"a single entry"));
                }

                if (*value).type_id() != type_meta.type_id()
                    && let Some(from_reflect) = type_meta.get_data::<ReflectFromReflect>()
                    && let Some(target_value) = from_reflect.from_reflect(&*value)
                {
                    return Ok(target_value);
                }

                Ok(value)
            }
        }

        deserializer.deserialize_map(ReflectDeserializeDriverVisitor {
            registry: self.registry,
            processor: self.processor,
        })
    }
}

/// A helper that resolves [`TypeMeta`] from a type-path string.
struct TypePathDeserializer<'a> {
    registry: &'a TypeRegistry,
}

impl<'a> TypePathDeserializer<'a> {
    /// Creates a new [`TypePathDeserializer`].
    #[inline]
    pub fn new(registry: &'a TypeRegistry) -> Self {
        Self { registry }
    }
}

impl<'a, 'de> DeserializeSeed<'de> for TypePathDeserializer<'a> {
    type Value = &'a TypeMeta;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        struct TypePathVisitor<'a>(&'a TypeRegistry);

        impl<'de, 'a> Visitor<'de> for TypePathVisitor<'a> {
            type Value = &'a TypeMeta;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("string containing `type` entry for the reflected value")
            }

            fn visit_str<E: Error>(self, type_path: &str) -> Result<Self::Value, E> {
                self.0.get_by_path(type_path).ok_or_else(|| {
                    Error::custom(format!("no registration found for `{type_path}`"))
                })
            }
        }

        deserializer.deserialize_str(TypePathVisitor(self.registry))
    }
}
