use alloc::format;

use serde_core::ser::SerializeTupleStruct;
use serde_core::{Serialize, Serializer};

use super::error_utils::make_custom_error;
use super::{SerializeDriver, SerializeProcessor};
use crate::info::TypeInfo;
use crate::ops::TupleStruct;
use crate::registry::TypeRegistry;

/// A serializer for [`TupleStruct`] values.
pub(super) struct TupleStructSerializer<'a, P: SerializeProcessor> {
    pub tuple_struct: &'a dyn TupleStruct,
    pub registry: &'a TypeRegistry,
    pub processor: Option<&'a P>,
}

impl<P: SerializeProcessor> Serialize for TupleStructSerializer<'_, P> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Some(type_info) = self.tuple_struct.represented_type_info() else {
            return Err(make_custom_error(format! {
                "missing represented type info for `{}`",
                self.tuple_struct.reflect_type_path()
            }));
        };

        let TypeInfo::TupleStruct(tuple_struct_info) = type_info else {
            return Err(make_custom_error(format!(
                "expected tuple struct but received {type_info:?}"
            )));
        };

        if self.tuple_struct.field_len() != tuple_struct_info.field_len() {
            return Err(make_custom_error(format!(
                "Field count mismatch: expect `{}` has {} fields, actual `{}` has {} fields",
                tuple_struct_info.type_path(),
                tuple_struct_info.field_len(),
                self.tuple_struct.reflect_type_path(),
                self.tuple_struct.field_len(),
            )));
        }

        let type_ident = tuple_struct_info.type_ident();
        let field_len = tuple_struct_info.field_len();
        let serde_len = tuple_struct_info.iter().filter(|f| !f.skip_serde()).count();

        if field_len == 1 && serde_len == 1 {
            let value = self.tuple_struct.field(0).unwrap();
            serializer.serialize_newtype_struct(
                type_ident,
                &SerializeDriver::new_internal(value, self.registry, self.processor),
            )
        } else {
            let mut state = serializer.serialize_tuple_struct(type_ident, serde_len)?;

            for index in tuple_struct_info
                .iter()
                .filter_map(|f| (!f.skip_serde()).then_some(f.index()))
            {
                let value = self.tuple_struct.field(index).unwrap();
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
