use serde_core::ser::SerializeTuple;
use serde_core::{Serialize, Serializer};

use super::{SerializeDriver, SerializeProcessor};

use crate::ops::Tuple;
use crate::registry::TypeRegistry;

/// A serializer for [`Tuple`] values.
pub(super) struct TupleSerializer<'a, P: SerializeProcessor> {
    pub tuple: &'a dyn Tuple,
    pub registry: &'a TypeRegistry,
    pub processor: Option<&'a P>,
}

impl<P: SerializeProcessor> Serialize for TupleSerializer<'_, P> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_tuple(self.tuple.field_len())?;

        for value in self.tuple.iter_fields() {
            state.serialize_element(&SerializeDriver::new_internal(
                value,
                self.registry,
                self.processor,
            ))?;
        }
        state.end()
    }
}
