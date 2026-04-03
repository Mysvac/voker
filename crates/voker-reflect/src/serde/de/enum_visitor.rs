use alloc::format;
use core::fmt::{self, Formatter};

use serde_core::Deserializer;
use serde_core::de::{DeserializeSeed, Error, Visitor};
use serde_core::de::{EnumAccess, MapAccess, SeqAccess, VariantAccess};

use super::error_utils::make_custom_error;
use super::struct_like_utils::{visit_struct, visit_struct_seq};
use super::tuple_like_utils::{TupleLikeInfo, visit_tuple};
use super::{DeserializeDriver, DeserializeProcessor};

use crate::info::{EnumInfo, StructVariantInfo, TupleVariantInfo, VariantInfo};
use crate::ops::{DynamicEnum, DynamicStruct, DynamicTuple, DynamicVariant};
use crate::registry::TypeRegistry;

// -----------------------------------------------------------------------------
// Enum Visitor

/// A [`Visitor`] for deserializing [`Enum`] values.
///
/// [`Enum`]: crate::Enum
pub(super) struct EnumVisitor<'a, P: DeserializeProcessor> {
    pub enum_info: &'static EnumInfo,
    pub registry: &'a TypeRegistry,
    pub processor: Option<&'a mut P>,
}

impl<'de, P: DeserializeProcessor> Visitor<'de> for EnumVisitor<'_, P> {
    type Value = DynamicEnum;

    fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
        formatter.write_str("reflected enum value")
    }

    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where
        A: EnumAccess<'de>,
    {
        let (variant_info, variant) = data.variant_seed(VariantDeserializer {
            enum_info: self.enum_info,
        })?;

        let value: DynamicVariant = match variant_info {
            VariantInfo::Unit(_) => variant.unit_variant()?.into(),
            VariantInfo::Struct(info) => variant
                .struct_variant(
                    info.field_names(),
                    StructVariantVisitor {
                        struct_info: info,
                        registry: self.registry,
                        processor: self.processor,
                    },
                )?
                .into(),
            VariantInfo::Tuple(info) => {
                if info.field_len() == 1 && !info.field(0).unwrap().skip_serde() {
                    let field = TupleLikeInfo::field(info, 0)?;
                    let Some(type_meta) = self.registry.get(field.type_id()) else {
                        return Err(make_custom_error(format!(
                            "no TypeMeta found for type `{}`",
                            field.type_info().type_path(),
                        )));
                    };

                    let value = variant.newtype_variant_seed(DeserializeDriver::new_internal(
                        type_meta,
                        self.registry,
                        self.processor,
                    ))?;
                    let mut dynamic = DynamicTuple::with_capacity(1);
                    dynamic.extend_boxed(value);
                    dynamic.into()
                } else {
                    let dynamic = variant.tuple_variant(
                        info.field_len(),
                        TupleVariantVisitor {
                            tuple_info: info,
                            registry: self.registry,
                            processor: self.processor,
                        },
                    )?;
                    dynamic.into()
                }
            }
        };
        let variant_name = variant_info.name();
        let variant_index = self.enum_info.index_of(variant_name).unwrap();
        let dynamic_enum = DynamicEnum::new(variant_index, variant_name, value);

        Ok(dynamic_enum)
    }
}

// -----------------------------------------------------------------------------
// Variant Visitor

struct VariantDeserializer {
    enum_info: &'static EnumInfo,
}

impl<'de> DeserializeSeed<'de> for VariantDeserializer {
    type Value = &'static VariantInfo;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        struct VariantVisitor(&'static EnumInfo);

        impl<'de> Visitor<'de> for VariantVisitor {
            type Value = &'static VariantInfo;

            fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
                formatter.write_str("expected either a variant index or variant name")
            }

            fn visit_u32<E>(self, variant_index: u32) -> Result<Self::Value, E>
            where
                E: Error,
            {
                match self.0.variant_at(variant_index as usize) {
                    Some(val) => Ok(val),
                    None => Err(make_custom_error(format!(
                        "no variant found at index `{}` on enum `{}`",
                        variant_index,
                        self.0.type_path()
                    ))),
                }
            }

            fn visit_str<E>(self, variant_name: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                match self.0.variant(variant_name) {
                    Some(val) => Ok(val),
                    None => Err(make_custom_error(format!(
                        "no variant found with name `{}` on enum `{}`",
                        variant_name,
                        self.0.type_path()
                    ))),
                }
            }
        }

        deserializer.deserialize_identifier(VariantVisitor(self.enum_info))
    }
}

struct StructVariantVisitor<'a, P: DeserializeProcessor> {
    struct_info: &'static StructVariantInfo,
    registry: &'a TypeRegistry,
    processor: Option<&'a mut P>,
}

impl<'de, P: DeserializeProcessor> Visitor<'de> for StructVariantVisitor<'_, P> {
    type Value = DynamicStruct;

    fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
        formatter.write_str("reflected struct variant value")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        visit_struct_seq(&mut seq, self.struct_info, self.registry, self.processor)
    }

    fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
    where
        V: MapAccess<'de>,
    {
        visit_struct(&mut map, self.struct_info, self.registry, self.processor)
    }
}

struct TupleVariantVisitor<'a, P: DeserializeProcessor> {
    tuple_info: &'static TupleVariantInfo,
    registry: &'a TypeRegistry,
    processor: Option<&'a mut P>,
}

impl<'de, P: DeserializeProcessor> Visitor<'de> for TupleVariantVisitor<'_, P> {
    type Value = DynamicTuple;

    fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
        formatter.write_str("reflected tuple variant value")
    }

    fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
    where
        V: SeqAccess<'de>,
    {
        visit_tuple(&mut seq, self.tuple_info, self.registry, self.processor)
    }
}
