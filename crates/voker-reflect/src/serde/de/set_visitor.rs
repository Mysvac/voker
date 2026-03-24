use alloc::format;
use core::fmt::{self, Formatter};

use serde_core::de::{SeqAccess, Visitor};

use super::error_utils::make_custom_error;
use super::{DeserializeDriver, DeserializeProcessor};

use crate::info::SetInfo;
use crate::ops::DynamicSet;
use crate::registry::TypeRegistry;

/// A [`Visitor`] for deserializing [`Set`] values.
///
/// [`Set`]: crate::ops::Set
pub(super) struct SetVisitor<'a, P: DeserializeProcessor> {
    pub set_info: &'static SetInfo,
    pub registry: &'a TypeRegistry,
    pub processor: Option<&'a mut P>,
}

impl<'de, P: DeserializeProcessor> Visitor<'de> for SetVisitor<'_, P> {
    type Value = DynamicSet;

    fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
        formatter.write_str("reflected set value")
    }

    fn visit_seq<V>(mut self, mut set: V) -> Result<Self::Value, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let Some(type_meta) = self.registry.get(self.set_info.value_id()) else {
            return Err(make_custom_error(format!(
                "no TypeMeta found for type `{}`",
                self.set_info.value_info().type_path(),
            )));
        };

        let capacity_hint = set.size_hint().unwrap_or_default();
        let mut dynamic = DynamicSet::with_capacity(capacity_hint);

        while let Some(value) = set.next_element_seed(DeserializeDriver::new_internal(
            type_meta,
            self.registry,
            self.processor.as_deref_mut(),
        ))? {
            dynamic.extend_boxed(value);
        }

        Ok(dynamic)
    }
}
