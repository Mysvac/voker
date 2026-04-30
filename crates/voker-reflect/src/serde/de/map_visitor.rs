use alloc::format;
use core::fmt::{self, Formatter};

use serde_core::de::{MapAccess, Visitor};

use super::error_utils::make_custom_error;
use super::{DeserializeDriver, DeserializeProcessor};
use crate::info::MapInfo;
use crate::ops::DynamicMap;
use crate::registry::TypeRegistry;

/// A [`Visitor`] for deserializing [`Map`] values.
///
/// [`Map`]: crate::ops::Map
pub(super) struct MapVisitor<'a, P: DeserializeProcessor> {
    pub map_info: &'static MapInfo,
    pub registry: &'a TypeRegistry,
    pub processor: Option<&'a mut P>,
}

impl<'de, P: DeserializeProcessor> Visitor<'de> for MapVisitor<'_, P> {
    type Value = DynamicMap;

    fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
        formatter.write_str("reflected map value")
    }

    fn visit_map<V>(mut self, mut map: V) -> Result<Self::Value, V::Error>
    where
        V: MapAccess<'de>,
    {
        let Some(key_meta) = self.registry.get(self.map_info.key_id()) else {
            return Err(make_custom_error(format!(
                "no TypeMeta found for type `{}`",
                self.map_info.key_info().type_path(),
            )));
        };
        let Some(value_meta) = self.registry.get(self.map_info.value_id()) else {
            return Err(make_custom_error(format!(
                "no TypeMeta found for type `{}`",
                self.map_info.value_info().type_path(),
            )));
        };

        let capacity_hint = map.size_hint().unwrap_or_default();
        let mut dynamic = DynamicMap::with_capacity(capacity_hint);

        while let Some(key) = map.next_key_seed(DeserializeDriver::new_internal(
            key_meta,
            self.registry,
            self.processor.as_deref_mut(),
        ))? {
            let value = map.next_value_seed(DeserializeDriver::new_internal(
                value_meta,
                self.registry,
                self.processor.as_deref_mut(),
            ))?;

            dynamic.extend_boxed(key, value);
        }

        Ok(dynamic)
    }
}
