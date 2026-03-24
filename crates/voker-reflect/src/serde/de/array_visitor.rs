use alloc::format;
use core::fmt::{self, Formatter};

use serde_core::de::{SeqAccess, Visitor};

use super::error_utils::make_custom_error;
use super::{DeserializeDriver, DeserializeProcessor};

use crate::info::ArrayInfo;
use crate::ops::{Array, DynamicArray};
use crate::registry::TypeRegistry;

/// A [`Visitor`] for deserializing [`Array`] values.
///
/// [`Array`]: crate::ops::Array
pub(super) struct ArrayVisitor<'a, P: DeserializeProcessor> {
    pub array_info: &'static ArrayInfo,
    pub registry: &'a TypeRegistry,
    pub processor: Option<&'a mut P>,
}

impl<'de, P: DeserializeProcessor> Visitor<'de> for ArrayVisitor<'_, P> {
    type Value = DynamicArray;

    fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
        formatter.write_str("reflected array value")
    }

    fn visit_seq<V>(mut self, mut seq: V) -> Result<Self::Value, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let Some(type_meta) = self.registry.get(self.array_info.item_id()) else {
            return Err(make_custom_error(format!(
                "no TypeMeta found for type `{}`",
                self.array_info.item_info().type_path()
            )));
        };

        let mut dynamic = DynamicArray::with_capacity(self.array_info.len());

        while let Some(value) = seq.next_element_seed(DeserializeDriver::new_internal(
            type_meta,
            self.registry,
            self.processor.as_deref_mut(),
        ))? {
            dynamic.extend_boxed(value);
        }

        if dynamic.len() != self.array_info.len() {
            return Err(make_custom_error(format!(
                "invalid length, expected: `{}`, actual: `{}`.",
                self.array_info.len(),
                dynamic.len(),
            )));
        }

        Ok(dynamic)
    }
}
