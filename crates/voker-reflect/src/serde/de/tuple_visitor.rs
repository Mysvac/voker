use core::fmt::{self, Formatter};
use serde_core::de::{SeqAccess, Visitor};

use super::DeserializeProcessor;
use super::tuple_like_utils::visit_tuple;

use crate::info::TupleInfo;
use crate::ops::DynamicTuple;
use crate::registry::TypeRegistry;

/// A [`Visitor`] for deserializing [`Tuple`] values.
///
/// [`Tuple`]: crate::Tuple
pub(super) struct TupleVisitor<'a, P: DeserializeProcessor> {
    pub tuple_info: &'static TupleInfo,
    pub registry: &'a TypeRegistry,
    pub processor: Option<&'a mut P>,
}

impl<'de, P: DeserializeProcessor> Visitor<'de> for TupleVisitor<'_, P> {
    type Value = DynamicTuple;

    fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
        formatter.write_str("reflected tuple value")
    }

    fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
    where
        V: SeqAccess<'de>,
    {
        visit_tuple(&mut seq, self.tuple_info, self.registry, self.processor)
    }
}
