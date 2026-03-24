use alloc::format;

use serde_core::ser::{SerializeStructVariant, SerializeTupleVariant};
use serde_core::{Serialize, Serializer};

use super::error_utils::make_custom_error;
use super::{SerializeDriver, SerializeProcessor};

use crate::info::{TypeInfo, VariantInfo};
use crate::ops::Enum;
use crate::registry::TypeRegistry;

/// A serializer for [`Enum`] values.
pub(super) struct EnumSerializer<'a, P: SerializeProcessor> {
    pub enum_value: &'a dyn Enum,
    pub registry: &'a TypeRegistry,
    pub processor: Option<&'a P>,
}

impl<P: SerializeProcessor> Serialize for EnumSerializer<'_, P> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Some(type_info) = self.enum_value.represented_type_info() else {
            return Err(make_custom_error(format! {
                "missing represented type info for `{}`",
                self.enum_value.reflect_type_path()
            }));
        };

        let TypeInfo::Enum(enum_info) = type_info else {
            return Err(make_custom_error(format!(
                "expected enum but received {type_info:?}"
            )));
        };

        let variant_index = self.enum_value.variant_index() as u32;
        let Some(variant_info) = enum_info.variant_at(variant_index as usize) else {
            return Err(make_custom_error(format!(
                "variant index `{variant_index}` does not exist for `{}`",
                enum_info.type_path(),
            )));
        };

        if self.enum_value.field_len() != variant_info.field_len() {
            return Err(make_custom_error(format!(
                "Field count mismatch: expect `{}::{}` has {} fields, actual `{}::{}` has {} fields",
                enum_info.type_path(),
                variant_info.name(),
                variant_info.field_len(),
                self.enum_value.reflect_type_path(),
                self.enum_value.variant_name(),
                self.enum_value.field_len(),
            )));
        }

        match variant_info {
            VariantInfo::Unit(info) => {
                let enum_name = enum_info.type_ident();
                if enum_name == "Option" && enum_info.module_path() == Some("core::option") {
                    serializer.serialize_none()
                } else {
                    let variant_name = info.name();
                    serializer.serialize_unit_variant(enum_name, variant_index, variant_name)
                }
            }
            VariantInfo::Struct(info) => {
                let enum_name = enum_info.type_ident();
                let variant_name = info.name();
                let serde_len = info.iter().filter(|f| !f.skip_serde()).count();

                let mut state = serializer.serialize_struct_variant(
                    enum_name,
                    variant_index,
                    variant_name,
                    serde_len,
                )?;

                for name in info
                    .iter()
                    .filter_map(|f| (!f.skip_serde()).then_some(f.name()))
                {
                    // If fields match in type and count but a field is missing, panic directly.
                    let value = self.enum_value.field(name).unwrap();
                    state.serialize_field(
                        name,
                        &SerializeDriver::new_internal(value, self.registry, self.processor),
                    )?;
                }

                state.end()
            }
            VariantInfo::Tuple(info) => {
                let enum_name = enum_info.type_ident();
                let variant_name = info.name();
                let field_len = info.field_len();
                let serde_len = info.iter().filter(|f| !f.skip_serde()).count();

                if field_len == 1 && serde_len == 1 {
                    let value = self.enum_value.field_at(0).unwrap();
                    if enum_name == "Option" && enum_info.module_path() == Some("core::option") {
                        serializer.serialize_some(&SerializeDriver::new_internal(
                            value,
                            self.registry,
                            self.processor,
                        ))
                    } else {
                        serializer.serialize_newtype_variant(
                            enum_name,
                            variant_index,
                            variant_name,
                            &SerializeDriver::new_internal(value, self.registry, self.processor),
                        )
                    }
                } else {
                    let mut state = serializer.serialize_tuple_variant(
                        enum_name,
                        variant_index,
                        variant_name,
                        serde_len,
                    )?;

                    for index in info
                        .iter()
                        .filter_map(|f| (!f.skip_serde()).then_some(f.index()))
                    {
                        let value = self.enum_value.field_at(index).unwrap();
                        state.serialize_field(&SerializeDriver::new_internal(
                            value,
                            self.registry,
                            self.processor,
                        ))?;
                    }

                    state.end()
                }
            }
        }
    }
}
