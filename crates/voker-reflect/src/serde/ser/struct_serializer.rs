use alloc::format;

use serde_core::ser::SerializeStruct;
use serde_core::{Serialize, Serializer};

use super::error_utils::make_custom_error;
use super::{SerializeDriver, SerializeProcessor};

use crate::info::TypeInfo;
use crate::ops::Struct;
use crate::registry::TypeRegistry;

/// A serializer for [`Struct`] values.
pub(super) struct StructSerializer<'a, P: SerializeProcessor> {
    pub struct_value: &'a dyn Struct,
    pub registry: &'a TypeRegistry,
    pub processor: Option<&'a P>,
}

impl<P: SerializeProcessor> Serialize for StructSerializer<'_, P> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Some(type_info) = self.struct_value.represented_type_info() else {
            return Err(make_custom_error(format! {
                "missing represented type info for `{}`",
                self.struct_value.reflect_type_path()
            }));
        };

        let TypeInfo::Struct(struct_info) = type_info else {
            return Err(make_custom_error(format!(
                "expected struct but received {type_info:?}"
            )));
        };

        if self.struct_value.field_len() != struct_info.field_len() {
            return Err(make_custom_error(format!(
                "Field count mismatch: expect `{}` has {} fields, actual `{}` has {} fields",
                struct_info.type_path(),
                struct_info.field_len(),
                self.struct_value.reflect_type_path(),
                self.struct_value.field_len(),
            )));
        }

        let type_ident = struct_info.type_ident();
        let serde_len = struct_info.iter().filter(|f| !f.skip_serde()).count();

        let mut state = serializer.serialize_struct(type_ident, serde_len)?;

        for name in struct_info
            .iter()
            .filter_map(|f| (!f.skip_serde()).then_some(f.name()))
        {
            // If fields match in type and count but a field is missing, panic directly.
            let value = self.struct_value.field(name).unwrap();
            state.serialize_field(
                name,
                &SerializeDriver::new_internal(value, self.registry, self.processor),
            )?;
        }

        state.end()
    }
}
