use alloc::format;
use core::fmt::{self, Formatter};

use serde_core::Deserializer;
use serde_core::de::{DeserializeSeed, SeqAccess, Visitor};

use super::error_utils::make_custom_error;
use super::tuple_like_utils::visit_tuple;
use super::{DeserializeDriver, DeserializeProcessor};
use crate::info::TupleStructInfo;
use crate::ops::{DynamicTuple, DynamicTupleStruct};
use crate::registry::TypeRegistry;

/// A [`Visitor`] for deserializing [`TupleStruct`] values.
///
/// [`TupleStruct`]: crate::TupleStruct
pub(super) struct TupleStructVisitor<'a, P: DeserializeProcessor> {
    pub tuple_struct_info: &'static TupleStructInfo,
    pub registry: &'a TypeRegistry,
    pub processor: Option<&'a mut P>,
}

impl<'de, P: DeserializeProcessor> Visitor<'de> for TupleStructVisitor<'_, P> {
    type Value = DynamicTupleStruct;

    fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
        formatter.write_str("reflected tuple struct value")
    }

    fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
    where
        V: SeqAccess<'de>,
    {
        visit_tuple(
            &mut seq,
            self.tuple_struct_info,
            self.registry,
            self.processor,
        )
        .map(DynamicTuple::into)
    }

    fn visit_newtype_struct<D: Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error> {
        let field = self.tuple_struct_info.field(0).unwrap();

        // If the length is `1` and the field is `skip_serde`,
        // it should call 'visit_tuple' instead of 'visit_newtype_struct'.
        assert!(self.tuple_struct_info.field_len() == 1 && !field.skip_serde());

        let Some(type_meta) = self.registry.get(field.type_id()) else {
            return Err(make_custom_error(format!(
                "no TypeMeta found for type `{}`",
                field.type_info().type_path(),
            )));
        };

        let mut dynamic = DynamicTupleStruct::with_capacity(1);

        let de = DeserializeDriver::new_internal(type_meta, self.registry, self.processor);
        let value = de.deserialize(deserializer)?;

        dynamic.extend_boxed(value);

        Ok(dynamic)
    }
}
