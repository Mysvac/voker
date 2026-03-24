use core::{fmt, fmt::Formatter};

use serde_core::de::{MapAccess, SeqAccess, Visitor};

use super::DeserializeProcessor;
use super::struct_like_utils::{visit_struct, visit_struct_seq};

use crate::info::StructInfo;
use crate::ops::DynamicStruct;
use crate::registry::TypeRegistry;

/// A [`Visitor`] for deserializing [`Struct`] values.
///
/// [`Struct`]: crate::ops::Struct
pub(super) struct StructVisitor<'a, P: DeserializeProcessor> {
    pub struct_info: &'static StructInfo,
    pub registry: &'a TypeRegistry,
    pub processor: Option<&'a mut P>,
}

impl<'de, P: DeserializeProcessor> Visitor<'de> for StructVisitor<'_, P> {
    type Value = DynamicStruct;

    fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
        formatter.write_str("reflected struct value")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        visit_struct_seq(&mut seq, self.struct_info, self.registry, self.processor)
    }

    fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
    where
        V: MapAccess<'de>,
    {
        visit_struct(&mut map, self.struct_info, self.registry, self.processor)
    }
}
