use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use core::fmt;

use serde_core::de::{Error, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_core::{Deserialize, Deserializer};
use voker_utils::hash::HashMap;

use super::error_utils::make_custom_error;
use super::{DeserializeDriver, DeserializeProcessor};

use crate::Reflect;
use crate::info::{NamedField, StructInfo, StructVariantInfo};
use crate::ops::DynamicStruct;
use crate::registry::{ReflectDefault, TypeRegistry};

// -----------------------------------------------------------------------------
// Struct-like metadata access

/// A helper trait for accessing type information from struct-like types.
pub(super) trait StructLikeInfo {
    fn name(&self) -> &'static str;
    fn field<E: Error>(&self, name: &str) -> Result<&NamedField, E>;
    fn field_at<E: Error>(&self, index: usize) -> Result<&NamedField, E>;
    fn field_len(&self) -> usize;
}

impl StructLikeInfo for StructInfo {
    fn name(&self) -> &'static str {
        self.type_path()
    }

    fn field<E: Error>(&self, name: &str) -> Result<&NamedField, E> {
        <Self>::field(self, name).ok_or_else(|| {
            Error::custom(format!(
                "no field named `{}` on struct `{}`",
                name,
                self.type_path(),
            ))
        })
    }

    fn field_at<E: Error>(&self, index: usize) -> Result<&NamedField, E> {
        <Self>::field_at(self, index).ok_or_else(|| {
            Error::custom(format!(
                "no field at index `{}` on struct `{}`",
                index,
                self.type_path(),
            ))
        })
    }

    #[inline]
    fn field_len(&self) -> usize {
        <Self>::field_len(self)
    }
}

impl StructLikeInfo for StructVariantInfo {
    fn name(&self) -> &'static str {
        <Self>::name(self)
    }

    fn field<E: Error>(&self, name: &str) -> Result<&NamedField, E> {
        <Self>::field(self, name).ok_or_else(|| {
            Error::custom(format!(
                "no field named `{}` on variant `{}`",
                name,
                self.name(),
            ))
        })
    }

    fn field_at<E: Error>(&self, index: usize) -> Result<&NamedField, E> {
        <Self>::field_at(self, index).ok_or_else(|| {
            Error::custom(format!(
                "no field at index `{}` on variant `{}`",
                index,
                self.name(),
            ))
        })
    }

    #[inline]
    fn field_len(&self) -> usize {
        <Self>::field_len(self)
    }
}

// -----------------------------------------------------------------------------
// Ident parser

struct Ident(pub String);

impl<'de> Deserialize<'de> for Ident {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct IdentVisitor;

        impl<'de> Visitor<'de> for IdentVisitor {
            type Value = Ident;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("identifier")
            }

            #[inline]
            fn visit_str<E: Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(Ident(String::from(value)))
            }

            #[inline]
            fn visit_string<E: Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(Ident(value))
            }
        }

        deserializer.deserialize_identifier(IdentVisitor)
    }
}

// -----------------------------------------------------------------------------
// struct visitor

/// Deserializes a [struct-like] type from a mapping of fields, returning a [`DynamicStruct`].
///
/// [struct-like]: StructLikeInfo
pub(super) fn visit_struct<'de, T, V, P>(
    map: &mut V,
    info: &'static T,
    registry: &TypeRegistry,
    mut processor: Option<&mut P>,
) -> Result<DynamicStruct, V::Error>
where
    T: StructLikeInfo,
    V: MapAccess<'de>,
    P: DeserializeProcessor,
{
    let field_len = info.field_len();
    let mut buffer: HashMap<String, Box<dyn Reflect>> = HashMap::with_capacity(field_len);

    while let Some(Ident(key)) = map.next_key::<Ident>()? {
        let field = info.field::<V::Error>(&key)?;
        let Some(type_meta) = registry.get(field.type_id()) else {
            return Err(make_custom_error(format!(
                "no TypeMeta found for type `{}`",
                field.type_info().type_path(),
            )));
        };
        let value = map.next_value_seed(DeserializeDriver::new_internal(
            type_meta,
            registry,
            processor.as_deref_mut(),
        ))?;
        buffer.insert(key, value);
    }

    let mut dynamic = DynamicStruct::with_capacity(field_len);

    for index in 0..field_len {
        let field = info.field_at::<V::Error>(index)?;
        let field_name: &'static str = field.name();

        if let Some(value) = buffer.remove(field_name) {
            dynamic.extend_boxed(field_name, value);
        } else if field.skip_serde() {
            if let Some(ctor) = registry.get_type_trait::<ReflectDefault>(field.type_id()) {
                dynamic.extend_boxed(field_name, ctor.default());
                continue;
            } else {
                return Err(make_custom_error(format!(
                    "field `{field_name}: {}` on `{}` is `skip_serde` but does not provide `ReflectDefault`",
                    field.type_info().type_path(),
                    info.name(),
                )));
            }
        } else {
            return Err(make_custom_error(format!(
                "missing field `{field_name}: {}` on `{}`",
                field.type_info().type_path(),
                info.name(),
            )));
        }
    }

    Ok(dynamic)
}

/// Deserializes a [struct-like] type from a sequence of fields, returning a [`DynamicStruct`].
///
/// [struct-like]: StructLikeInfo
pub(super) fn visit_struct_seq<'de, T, V, P>(
    seq: &mut V,
    info: &T,
    registry: &TypeRegistry,
    mut processor: Option<&mut P>,
) -> Result<DynamicStruct, V::Error>
where
    T: StructLikeInfo,
    V: SeqAccess<'de>,
    P: DeserializeProcessor,
{
    let len = info.field_len();
    let mut dynamic = DynamicStruct::with_capacity(len);

    for index in 0..len {
        let field = info.field_at::<V::Error>(index)?;
        let field_name = field.name();

        if field.skip_serde() {
            if let Some(ctor) = registry.get_type_trait::<ReflectDefault>(field.type_id()) {
                dynamic.extend_boxed(field_name, ctor.default());
                continue;
            } else {
                return Err(make_custom_error(format!(
                    "field `{field_name}: {}` on `{}` is `skip_serde` but does not provide `ReflectDefault`",
                    field.type_info().type_path(),
                    info.name(),
                )));
            }
        }

        let Some(type_meta) = registry.get(field.type_id()) else {
            return Err(make_custom_error(format!(
                "no TypeMeta found for type `{}`",
                field.type_info().type_path(),
            )));
        };

        let value = seq.next_element_seed(DeserializeDriver::new_internal(
            type_meta,
            registry,
            processor.as_deref_mut(),
        ))?;

        let Some(value) = value else {
            return Err(make_custom_error(format!(
                "invalid length for `{}`, expected: `{}`, actual: `{}`",
                info.name(),
                len,
                index,
            )));
        };

        dynamic.extend_boxed(field_name, value);
    }

    if seq.next_element::<IgnoredAny>()?.is_some() {
        return Err(make_custom_error(format!(
            "invalid length for `{}`, expected: `{}`, actual: `>{}`",
            info.name(),
            len,
            len,
        )));
    }

    Ok(dynamic)
}
