use alloc::boxed::Box;
use alloc::string::String;

use voker_os::sync::Arc;
use voker_utils::hash::HashMap;

use crate::info::{CustomAttributes, Generics, Type, TypePath, VariantInfo};
use crate::info::{impl_custom_attributes_fn, impl_with_custom_attributes};
use crate::info::{impl_docs_fn, impl_generic_fn, impl_type_fn};
use crate::ops::Enum;

/// A container for compile-time enum info, size = 120 (exclude `docs`).
///
/// # Examples
///
/// ```rust
/// use voker_reflect::info::{Typed, EnumInfo};
///
/// let info = <Option<i32> as Typed>::type_info().as_enum().unwrap();
/// assert!(info.variant("None").is_some());
/// ```
#[derive(Clone, Debug)]
pub struct EnumInfo {
    ty: Type,
    generics: Generics,
    variants: HashMap<&'static str, VariantInfo>,
    variant_names: Box<[&'static str]>,
    // Use `Option` to reduce unnecessary heap requests (when empty content).
    custom_attributes: Option<Arc<CustomAttributes>>,
    #[cfg(feature = "reflect_docs")]
    docs: Option<&'static str>,
}

impl EnumInfo {
    impl_type_fn!(ty);
    impl_docs_fn!(docs);
    impl_generic_fn!(generics);
    impl_custom_attributes_fn!(custom_attributes);
    impl_with_custom_attributes!(custom_attributes);

    /// Creates a new [`EnumInfo`].
    ///
    /// The order of internal variants is fixed, depends on the input order.
    pub fn new<TEnum: Enum + TypePath>(variants: &[VariantInfo]) -> Self {
        let variant_names = variants.iter().map(VariantInfo::name).collect();
        let variants = variants.iter().map(|v| (v.name(), v.clone())).collect();

        Self {
            ty: Type::of::<TEnum>(),
            generics: Generics::new(),
            variants,
            variant_names,
            custom_attributes: None,
            #[cfg(feature = "reflect_docs")]
            docs: None,
        }
    }

    /// Returns the [`VariantInfo`] for the given variant name, if present.
    pub fn variant(&self, name: &str) -> Option<&VariantInfo> {
        self.variants.get(name)
    }

    /// Returns the [`VariantInfo`] at the given index, if present.
    pub fn variant_at(&self, index: usize) -> Option<&VariantInfo> {
        self.variants.get(self.variant_names.get(index)?)
    }

    /// Returns an iterator over the variants in **declaration order**.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &VariantInfo> {
        self.variant_names.iter().map(|name| self.variants.get(name).unwrap()) // variants names should be valid
    }

    /// Returns the list of variant names in declaration order.
    #[inline]
    pub fn variant_names(&self) -> &[&'static str] {
        &self.variant_names
    }

    /// Returns the index for the given variant name, if present.
    ///
    /// This is O(N) complexity.
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.variant_names.iter().position(|s| *s == name)
    }

    /// Returns the full path for a variant name, e.g. `Type::Variant`.
    #[inline]
    pub fn variant_path(&self, name: &str) -> String {
        crate::impls::concat(&[self.type_path(), "::", name])
    }

    /// Returns the number of variants.
    #[inline]
    pub fn variant_len(&self) -> usize {
        self.variants.len()
    }
}
