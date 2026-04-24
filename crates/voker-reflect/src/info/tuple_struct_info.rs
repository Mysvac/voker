use alloc::boxed::Box;
use voker_os::Arc;

use crate::info::{CustomAttributes, Generics, Type, TypePath, UnnamedField};
use crate::info::{impl_custom_attributes_fn, impl_with_custom_attributes};
use crate::info::{impl_docs_fn, impl_generic_fn, impl_type_fn};
use crate::ops::TupleStruct;

/// A container for compile-time tuple-struct info, size = 88 (exclude `docs`).
///
/// # Examples
///
/// ```rust
/// use voker_reflect::{Reflect, info::Typed};
///
/// #[derive(Reflect)]
/// struct A(i32, String);
///
/// let info = <A as Typed>::type_info().as_tuple_struct().unwrap();
///
/// assert_eq!(info.field_len(), 2);
/// ```
#[derive(Clone, Debug)]
pub struct TupleStructInfo {
    ty: Type,
    generics: Generics,
    fields: Box<[UnnamedField]>,
    // Use `Option` to avoid allocating when there are no custom attributes.
    custom_attributes: Option<Arc<CustomAttributes>>,
    #[cfg(feature = "reflect_docs")]
    docs: Option<&'static str>,
}

impl TupleStructInfo {
    impl_docs_fn!(docs);
    impl_type_fn!(ty);
    impl_generic_fn!(generics);
    impl_custom_attributes_fn!(custom_attributes);
    impl_with_custom_attributes!(custom_attributes);

    /// Create a new [`TupleStructInfo`].
    ///
    /// The order of internal fields is fixed, depends on the input order.
    #[inline]
    pub fn new<T: TupleStruct + TypePath>(fields: &[UnnamedField]) -> Self {
        Self {
            ty: Type::of::<T>(),
            generics: Generics::new(),
            fields: fields.to_vec().into_boxed_slice(),
            custom_attributes: None,
            #[cfg(feature = "reflect_docs")]
            docs: None,
        }
    }

    /// Returns the [`UnnamedField`] at the given index, if present.
    #[inline]
    pub fn field(&self, index: usize) -> Option<&UnnamedField> {
        self.fields.get(index)
    }

    /// Returns an iterator over the fields in **declaration order**.
    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &UnnamedField> {
        self.fields.iter()
    }

    /// Returns the number of fields.
    #[inline]
    pub fn field_len(&self) -> usize {
        self.fields.len()
    }
}
