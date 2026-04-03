use alloc::format;

use serde_core::de::{Error, IgnoredAny, SeqAccess};

use super::error_utils::make_custom_error;
use super::{DeserializeDriver, DeserializeProcessor};

use crate::info::{TupleInfo, TupleStructInfo, TupleVariantInfo, UnnamedField};
use crate::ops::DynamicTuple;
use crate::registry::{ReflectDefault, TypeRegistry};

// -----------------------------------------------------------------------------
// Tuple-like metadata access

pub(super) trait TupleLikeInfo {
    fn name(&self) -> &'static str;
    fn field<E: Error>(&self, index: usize) -> Result<&UnnamedField, E>;
    fn field_len(&self) -> usize;
}

impl TupleLikeInfo for TupleInfo {
    fn name(&self) -> &'static str {
        self.type_path()
    }

    fn field<E: Error>(&self, index: usize) -> Result<&UnnamedField, E> {
        match <Self>::field(self, index) {
            Some(info) => Ok(info),
            None => Err(make_custom_error(format!(
                "no field at index `{}` on tuple `{}`",
                index,
                self.type_path(),
            ))),
        }
    }

    #[inline]
    fn field_len(&self) -> usize {
        <Self>::field_len(self)
    }
}

impl TupleLikeInfo for TupleStructInfo {
    fn name(&self) -> &'static str {
        self.type_path()
    }

    fn field<E: Error>(&self, index: usize) -> Result<&UnnamedField, E> {
        match <Self>::field(self, index) {
            Some(info) => Ok(info),
            None => Err(make_custom_error(format!(
                "no field at index `{}` on tuple-struct `{}`",
                index,
                self.type_path(),
            ))),
        }
    }

    #[inline]
    fn field_len(&self) -> usize {
        <Self>::field_len(self)
    }
}

impl TupleLikeInfo for TupleVariantInfo {
    fn name(&self) -> &'static str {
        <Self>::name(self)
    }

    fn field<E: Error>(&self, index: usize) -> Result<&UnnamedField, E> {
        match <Self>::field(self, index) {
            Some(info) => Ok(info),
            None => Err(make_custom_error(format!(
                "no field at index `{}` on tuple variant `{}`",
                index,
                self.name(),
            ))),
        }
    }

    #[inline]
    fn field_len(&self) -> usize {
        <Self>::field_len(self)
    }
}

// -----------------------------------------------------------------------------
// Tuple visitor

/// Deserializes a [tuple-like] type from a sequence of elements, returning a [`DynamicTuple`].
///
/// [tuple-like]: TupleLikeInfo
pub(super) fn visit_tuple<'de, T, V, P>(
    seq: &mut V,
    info: &T,
    registry: &TypeRegistry,
    mut processor: Option<&mut P>,
) -> Result<DynamicTuple, V::Error>
where
    T: TupleLikeInfo,
    V: SeqAccess<'de>,
    P: DeserializeProcessor,
{
    let len = info.field_len();
    let mut dynamic = DynamicTuple::with_capacity(len);

    for index in 0..len {
        let field = info.field::<V::Error>(index)?;

        if field.skip_serde() {
            if let Some(ctor) = registry.get_type_data::<ReflectDefault>(field.type_id()) {
                dynamic.extend_boxed(ctor.default());
                continue;
            } else {
                return Err(make_custom_error(format!(
                    "field `{index}: {}` on `{}` is `skip_serde` but does not provide `ReflectDefault`",
                    field.type_info().type_path(),
                    info.name(),
                )));
            }
        }

        let Some(type_meta) = registry.get(field.type_id()) else {
            return Err(make_custom_error(format!(
                "no TypeMeta found for type `{}`",
                field.type_info().type_path(),
            )));
        };

        let value = seq.next_element_seed(DeserializeDriver::new_internal(
            type_meta,
            registry,
            processor.as_deref_mut(),
        ))?;

        let Some(value) = value else {
            return Err(make_custom_error(format!(
                "invalid length for `{}`, expected: `{}`, actual: `{}`",
                info.name(),
                len,
                index,
            )));
        };

        dynamic.extend_boxed(value);
    }

    if seq.next_element::<IgnoredAny>()?.is_some() {
        return Err(make_custom_error(format!(
            "invalid length for `{}`, expected: `{}`, actual: `>{}`",
            info.name(),
            len,
            len,
        )));
    }

    Ok(dynamic)
}
