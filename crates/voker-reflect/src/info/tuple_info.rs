use alloc::boxed::Box;

use crate::info::{Generics, Type, TypePath, UnnamedField};
use crate::info::{impl_docs_fn, impl_generic_fn, impl_type_fn};
use crate::ops::Tuple;

/// A container for compile-time unnamed struct info.
///
/// At present, `ListInfo` does not have `CustomAttributes`.
/// If necessary, it may be added in the future.
///
/// # Examples
///
/// ```rust
/// use voker_reflect::info::{Typed, Type};
///
/// let info = <(i32, String) as Typed>::type_info().as_tuple().unwrap();
///
/// assert_eq!(info.field_len(), 2);
/// ```
#[derive(Clone, Debug)]
pub struct TupleInfo {
    ty: Type,
    generics: Generics,
    fields: Box<[UnnamedField]>,
    #[cfg(feature = "reflect_docs")]
    docs: Option<&'static str>,
}

impl TupleInfo {
    impl_docs_fn!(docs);
    impl_type_fn!(ty);
    impl_generic_fn!(generics);

    /// Create a new [`TupleInfo`].
    ///
    /// The order of internal fields is fixed, depends on the input order.
    #[inline]
    pub fn new<T: Tuple + TypePath>(fields: &[UnnamedField]) -> Self {
        Self {
            ty: Type::of::<T>(),
            generics: Generics::new(),
            fields: fields.to_vec().into_boxed_slice(),
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
    pub fn iter(&self) -> impl Iterator<Item = &UnnamedField> {
        self.fields.iter()
    }

    /// Returns the number of fields.
    #[inline]
    pub fn field_len(&self) -> usize {
        self.fields.len()
    }
}
