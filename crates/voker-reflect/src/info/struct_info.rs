use alloc::boxed::Box;

use voker_os::sync::Arc;
use voker_utils::hash::HashMap;

use crate::info::{CustomAttributes, Generics, NamedField, Type, TypePath};
use crate::info::{impl_custom_attributes_fn, impl_with_custom_attributes};
use crate::info::{impl_docs_fn, impl_generic_fn, impl_type_fn};
use crate::ops::Struct;

/// A container for compile-time named struct info.
///
/// # Examples
///
/// ```rust
/// use voker_reflect::{Reflect, info::{Typed, Type}};
///
/// #[derive(Reflect)]
/// struct A {
///     val: f32,
/// }
///
/// let info = <A as Typed>::type_info().as_struct().unwrap();
///
/// assert_eq!(info.field_len(), 1);
/// assert_eq!(info.index_of("val"), Some(0));
/// ```
#[derive(Clone, Debug)]
pub struct StructInfo {
    ty: Type,
    generics: Generics,
    fields: HashMap<&'static str, NamedField>,
    field_names: Box<[&'static str]>,
    // Use `Option` to reduce unnecessary heap requests (when empty content).
    custom_attributes: Option<Arc<CustomAttributes>>,
    #[cfg(feature = "reflect_docs")]
    docs: Option<&'static str>,
}

impl StructInfo {
    impl_docs_fn!(docs);
    impl_type_fn!(ty);
    impl_generic_fn!(generics);
    impl_custom_attributes_fn!(custom_attributes);
    impl_with_custom_attributes!(custom_attributes);

    /// Create a new [`StructInfo`].
    ///
    /// The order of internal fields is fixed, depends on the input order.
    pub fn new<T: Struct + TypePath>(fields: &[NamedField]) -> Self {
        let field_names = fields.iter().map(NamedField::name).collect();
        let fields = fields.iter().map(|v| (v.name(), v.clone())).collect();

        Self {
            ty: Type::of::<T>(),
            generics: Generics::new(),
            fields,
            field_names,
            custom_attributes: None,
            #[cfg(feature = "reflect_docs")]
            docs: None,
        }
    }

    /// Returns the [`NamedField`] for the given `name`, if present.
    pub fn field(&self, name: &str) -> Option<&NamedField> {
        self.fields.get(name)
    }

    /// Returns the [`NamedField`] at the given index, if present.
    pub fn field_at(&self, index: usize) -> Option<&NamedField> {
        self.fields.get(self.field_names.get(index)?)
    }

    /// Returns an iterator over the fields in **declaration order**.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &NamedField> {
        self.field_names.iter().map(|name| self.fields.get(name).unwrap()) // field names should be valid
    }

    /// Returns the field names in declaration order.
    #[inline]
    pub fn field_names(&self) -> &[&'static str] {
        &self.field_names
    }

    /// Returns the index for the given field `name`, if present.
    ///
    /// This is O(N) complexity.
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.field_names.iter().position(|s| *s == name)
    }

    /// Returns the number of fields.
    #[inline]
    pub fn field_len(&self) -> usize {
        self.field_names.len()
    }
}
