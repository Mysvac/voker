use serde_core::{Serialize, Serializer, ser::SerializeSeq};

use super::{SerializeDriver, SerializeProcessor};

use crate::ops::Set;
use crate::registry::TypeRegistry;

/// A serializer for [`Set`] values.
pub(super) struct SetSerializer<'a, P: SerializeProcessor> {
    pub set: &'a dyn Set,
    pub registry: &'a TypeRegistry,
    pub processor: Option<&'a P>,
}

impl<P: SerializeProcessor> Serialize for SetSerializer<'_, P> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_seq(Some(self.set.len()))?;
        for value in self.set.iter() {
            state.serialize_element(&SerializeDriver::new_internal(
                value,
                self.registry,
                self.processor,
            ))?;
        }
        state.end()
    }
}
