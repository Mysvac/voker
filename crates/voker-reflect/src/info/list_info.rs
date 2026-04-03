use core::any::{Any, TypeId};

use crate::Reflect;
use crate::info::{Generics, Type, TypeInfo, TypePath, Typed};
use crate::info::{impl_docs_fn, impl_generic_fn, impl_type_fn};
use crate::ops::List;

///  A container for compile-time list-like info.
///
/// At present, `ListInfo` does not have `CustomAttributes`.
/// If necessary, it may be added in the future.
///
/// # Examples
///
/// ```rust
/// # use core::any::TypeId;
/// use voker_reflect::info::Typed;
///
/// let info = <Vec<i32> as Typed>::type_info().as_list().unwrap();
///
/// assert_eq!(info.item_id(), TypeId::of::<i32>());
/// ```
#[derive(Clone, Debug)]
pub struct ListInfo {
    ty: Type,
    generics: Generics,
    item_id: TypeId,
    // `TypeInfo` is created on the first visit, use function pointers to delay it.
    item_info: fn() -> &'static TypeInfo,
    #[cfg(feature = "reflect_docs")]
    docs: Option<&'static str>,
}

impl ListInfo {
    impl_docs_fn!(docs);
    impl_type_fn!(ty);
    impl_generic_fn!(generics);

    /// Creates a new [`ListInfo`].
    #[inline]
    pub const fn new<TList: List + TypePath, TItem: Reflect + Typed>() -> Self {
        Self {
            ty: Type::of::<TList>(),
            generics: Generics::new(),
            item_id: TypeId::of::<TItem>(),
            item_info: TItem::type_info,
            #[cfg(feature = "reflect_docs")]
            docs: None,
        }
    }

    /// Returns the [`TypeId`] of list items.
    #[inline]
    pub const fn item_id(&self) -> TypeId {
        self.item_id
    }

    /// Returns return if the item type is `T`.
    #[inline]
    pub fn item_is<T: Any>(&self) -> bool {
        self.item_id == TypeId::of::<T>()
    }

    /// Returns the [`TypeInfo`] of list items.
    #[inline]
    pub fn item_info(&self) -> &'static TypeInfo {
        (self.item_info)()
    }
}
