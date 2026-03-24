use serde_core::{Serialize, Serializer, ser::SerializeSeq};

use super::{SerializeDriver, SerializeProcessor};

use crate::ops::List;
use crate::registry::TypeRegistry;

/// A serializer for [`List`] values.
pub(super) struct ListSerializer<'a, P: SerializeProcessor> {
    pub list: &'a dyn List,
    pub registry: &'a TypeRegistry,
    pub processor: Option<&'a P>,
}

impl<P: SerializeProcessor> Serialize for ListSerializer<'_, P> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_seq(Some(self.list.len()))?;
        for value in self.list.iter() {
            state.serialize_element(&SerializeDriver::new_internal(
                value,
                self.registry,
                self.processor,
            ))?;
        }
        state.end()
    }
}
