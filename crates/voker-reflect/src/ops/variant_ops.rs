use core::iter::FusedIterator;

use crate::Reflect;
use crate::info::VariantKind;
use crate::ops::{DynamicStruct, DynamicTuple, Enum, Struct, Tuple};

// -----------------------------------------------------------------------------
// Dynamic Variant

/// A dynamic representation of an enum variant.
///
/// This enum can represent any of the three possible enum variant types:
/// - **Unit variants**: no associated data (e.g., `Variant::Unit`)
/// - **Tuple variants**: ordered, unnamed fields (e.g., `Variant::Tuple(u32, String)`)
/// - **Struct variants**: named fields (e.g., `Variant::Struct { x: f32, y: f32 }`)
///
/// # Internal Representation
///
/// - **Unit variants**: no internal data
/// - **Tuple variants**: stored as [`DynamicTuple`]
/// - **Struct variants**: stored as [`DynamicStruct`]
///
/// # Creation
///
/// Use [`From::from`] to create a `DynamicVariant`:
/// - Unit variant: `DynamicVariant::from(())`
/// - Tuple variant: `DynamicVariant::from(dynamic_tuple)`
/// - Struct variant: `DynamicVariant::from(dynamic_struct)`
#[derive(Debug)]
pub enum DynamicVariant {
    Unit,
    Tuple(DynamicTuple),
    Struct(DynamicStruct),
}

impl Clone for DynamicVariant {
    fn clone(&self) -> Self {
        match self {
            Self::Unit => Self::Unit,
            Self::Tuple(data) => Self::Tuple(data.to_dynamic_tuple()),
            Self::Struct(data) => Self::Struct(data.to_dynamic_struct()),
        }
    }
}

impl From<()> for DynamicVariant {
    #[inline]
    fn from(_: ()) -> Self {
        Self::Unit
    }
}

impl From<DynamicTuple> for DynamicVariant {
    #[inline]
    fn from(value: DynamicTuple) -> Self {
        Self::Tuple(value)
    }
}

impl From<DynamicStruct> for DynamicVariant {
    #[inline]
    fn from(value: DynamicStruct) -> Self {
        Self::Struct(value)
    }
}

// -----------------------------------------------------------------------------
// Variant Field Iterator

/// A field in the current enum variant.
///
/// This enum represents a field within an enum variant, which can be either:
/// - A named field in a struct variant
/// - An unnamed field in a tuple variant
///
/// This provides a unified interface for accessing fields regardless of the
/// variant's kind.
pub enum VariantField<'a> {
    /// The name and value of a field in a struct variant.
    Struct(&'a str, &'a dyn Reflect),
    /// The value of a field in a tuple variant.
    Tuple(&'a dyn Reflect),
}

impl<'a> VariantField<'a> {
    /// Returns the name of a struct variant field, or `None` for a tuple variant field.
    #[inline]
    pub fn name(&self) -> Option<&'a str> {
        if let Self::Struct(name, ..) = self {
            Some(*name)
        } else {
            None
        }
    }

    /// Gets a reference to the value of this field.
    ///
    /// This works for both struct and tuple variant fields.
    #[inline]
    pub fn value(&self) -> &'a dyn Reflect {
        match *self {
            Self::Struct(_, value) | Self::Tuple(value) => value,
        }
    }
}

/// An iterator over the fields in the current enum variant.
///
/// This iterator yields [`VariantField`] items, which provide a unified
/// interface for accessing fields regardless of whether they come from a
/// struct variant (with names) or a tuple variant (without names).
///
/// The iterator respects the order of fields as defined in the enum variant.
pub struct VariantFieldIter<'a> {
    container: &'a dyn Enum,
    index: usize,
}

impl<'a> VariantFieldIter<'a> {
    /// Creates a new iterator for the given enum.
    #[inline(always)]
    pub const fn new(container: &'a dyn Enum) -> Self {
        Self {
            container,
            index: 0,
        }
    }
}

impl<'a> Iterator for VariantFieldIter<'a> {
    type Item = VariantField<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let value = match self.container.variant_kind() {
            VariantKind::Unit => None,
            VariantKind::Tuple => Some(VariantField::Tuple(self.container.field_at(self.index)?)),
            VariantKind::Struct => {
                let name = self.container.name_at(self.index)?;
                Some(VariantField::Struct(name, self.container.field(name)?))
            }
        };
        self.index += value.is_some() as usize;
        value
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let hint = self.container.field_len() - self.index;
        (hint, Some(hint))
    }
}

impl ExactSizeIterator for VariantFieldIter<'_> {}
impl FusedIterator for VariantFieldIter<'_> {}
