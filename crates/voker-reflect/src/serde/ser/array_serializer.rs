use serde_core::ser::SerializeTuple;
use serde_core::{Serialize, Serializer};

use super::{SerializeDriver, SerializeProcessor};

use crate::ops::Array;
use crate::registry::TypeRegistry;

/// A serializer for [`Array`] values.
pub(super) struct ArraySerializer<'a, P: SerializeProcessor> {
    pub array: &'a dyn Array,
    pub registry: &'a TypeRegistry,
    pub processor: Option<&'a P>,
}

impl<P: SerializeProcessor> Serialize for ArraySerializer<'_, P> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_tuple(self.array.len())?;
        for value in self.array.iter() {
            state.serialize_element(&SerializeDriver::new_internal(
                value,
                self.registry,
                self.processor,
            ))?;
        }
        state.end()
    }
}
