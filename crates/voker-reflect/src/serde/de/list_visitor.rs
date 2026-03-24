use alloc::format;
use core::fmt::{self, Formatter};

use serde_core::de::{SeqAccess, Visitor};

use super::error_utils::make_custom_error;
use super::{DeserializeDriver, DeserializeProcessor};

use crate::info::ListInfo;
use crate::ops::DynamicList;
use crate::registry::TypeRegistry;

/// A [`Visitor`] for deserializing [`List`] values.
///
/// [`List`]: crate::ops::List
pub(super) struct ListVisitor<'a, P: DeserializeProcessor> {
    pub list_info: &'static ListInfo,
    pub registry: &'a TypeRegistry,
    pub processor: Option<&'a mut P>,
}

impl<'de, P: DeserializeProcessor> Visitor<'de> for ListVisitor<'_, P> {
    type Value = DynamicList;

    fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
        formatter.write_str("reflected list value")
    }

    fn visit_seq<V>(mut self, mut seq: V) -> Result<Self::Value, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let Some(type_meta) = self.registry.get(self.list_info.item_id()) else {
            return Err(make_custom_error(format!(
                "no TypeMeta found for type `{}`",
                self.list_info.item_info().type_path()
            )));
        };

        let capacity_hint = seq.size_hint().unwrap_or_default();
        let mut dynamic = DynamicList::with_capacity(capacity_hint);

        while let Some(value) = seq.next_element_seed(DeserializeDriver::new_internal(
            type_meta,
            self.registry,
            self.processor.as_deref_mut(),
        ))? {
            dynamic.extend_boxed(value);
        }

        Ok(dynamic)
    }
}
