use serde_core::{Serialize, Serializer, ser::SerializeMap};

use super::{SerializeDriver, SerializeProcessor};

use crate::ops::Map;
use crate::registry::TypeRegistry;

/// A serializer for [`Map`] values.
pub(super) struct MapSerializer<'a, P: SerializeProcessor> {
    pub map: &'a dyn Map,
    pub registry: &'a TypeRegistry,
    pub processor: Option<&'a P>,
}

impl<P: SerializeProcessor> Serialize for MapSerializer<'_, P> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_map(Some(self.map.len()))?;
        for (key, value) in self.map.iter() {
            state.serialize_entry(
                &SerializeDriver::new_internal(key, self.registry, self.processor),
                &SerializeDriver::new_internal(value, self.registry, self.processor),
            )?;
        }
        state.end()
    }
}
