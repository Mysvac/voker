use core::any::{Any, TypeId};

use crate::Reflect;
use crate::info::{Generics, Type, TypeInfo, TypePath, Typed};
use crate::info::{impl_docs_fn, impl_generic_fn, impl_type_fn};
use crate::ops::Array;

/// A container for compile-time array information.
///
/// At present, `ArrayInfo` does not have `CustomAttributes`, which can save memory.
///
/// # Examples
///
/// ```
/// use voker_reflect::info::{Typed, ArrayInfo};
///
/// // Get the `ArrayInfo` for `[i32; 5]` and inspect its properties.
/// let info = <[i32; 5] as Typed>::type_info().as_array().unwrap();
///
/// assert_eq!(info.len(), 5);
/// assert_eq!(info.type_path(), "[i32; 5]");
///
/// let item_info = info.item_info();
/// assert!(item_info.type_is::<i32>());
/// ```
#[derive(Clone, Debug)]
pub struct ArrayInfo {
    ty: Type,
    generics: Generics,
    // Cache `TypeId` for deserialization.
    item_id: TypeId,
    // `TypeInfo` is created on the first visit, use function pointers to delay it.
    item_info: fn() -> &'static TypeInfo,
    len: usize,
    #[cfg(feature = "reflect_docs")]
    docs: Option<&'static str>,
}

impl ArrayInfo {
    impl_type_fn!(ty);
    impl_docs_fn!(docs);
    impl_generic_fn!(generics);

    /// Create a new [`ArrayInfo`].
    ///
    /// # Arguments
    ///
    /// - `len`: The length of the underlying array.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::info::ArrayInfo;
    /// let info = ArrayInfo::new::<[i32; 7], i32>(7);
    /// ```
    #[inline]
    pub const fn new<TArray: Array + TypePath, TItem: Reflect + Typed>(len: usize) -> Self {
        Self {
            ty: Type::of::<TArray>(),
            generics: Generics::new(),
            item_id: TypeId::of::<TItem>(),
            item_info: TItem::type_info,
            len,
            #[cfg(feature = "reflect_docs")]
            docs: None,
        }
    }

    /// The compile-time length of the array.
    #[inline]
    #[expect(clippy::len_without_is_empty, reason = "`Array::len` is fixed.")]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns the [`TypeId`] of an array item.
    #[inline]
    pub const fn item_id(&self) -> TypeId {
        self.item_id
    }

    /// Returns return if the item type is `T`.
    #[inline]
    pub fn item_is<T: Any>(&self) -> bool {
        self.item_id == TypeId::of::<T>()
    }

    /// Returns the [`TypeInfo`] of array items.
    #[inline]
    pub fn item_info(&self) -> &'static TypeInfo {
        (self.item_info)()
    }
}
