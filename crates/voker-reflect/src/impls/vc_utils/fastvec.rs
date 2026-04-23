use crate::derive::impl_type_path;

impl_type_path!(::voker_utils::vec::ArrayVec<T, const N: usize>);
impl_type_path!(::voker_utils::vec::FastVec<T, const N: usize>);
impl_type_path!(::voker_utils::vec::SmallVec<T, const N: usize>);

// -----------------------------------------------------------------------------
// SmallVec

use alloc::boxed::Box;
use alloc::vec::Vec;
use voker_utils::vec::SmallVec;

use crate::info::{ConstParamInfo, GenericInfo, Generics};
use crate::info::{ListInfo, TypeInfo, TypeParamInfo, Typed};
use crate::ops::{ApplyError, List, ListItemIter, ReflectCloneError};
use crate::registry::ReflectFromReflect;
use crate::registry::{FromType, GetTypeMeta, ReflectDefault, TypeMeta};
use crate::{FromReflect, Reflect, impls};

impl<T: Typed + FromReflect, const N: usize> Typed for SmallVec<T, N> {
    fn type_info() -> &'static TypeInfo {
        static CELL: impls::GenericTypeInfoCell = impls::GenericTypeInfoCell::new();
        CELL.get_or_insert::<Self>(|| {
            TypeInfo::List(ListInfo::new::<Self, T>().with_generics(Generics::from([
                GenericInfo::Type(TypeParamInfo::new::<T>("T")),
                GenericInfo::Const(ConstParamInfo::new("N", N)),
            ])))
        })
    }
}

impl<T: Typed + FromReflect, const N: usize> Reflect for SmallVec<T, N> {
    crate::reflection::impl_reflect_cast_fn!(List);

    #[inline]
    fn apply(&mut self, value: &dyn Reflect) -> Result<(), ApplyError> {
        impls::list_apply(self, value)
    }

    fn reflect_clone(&self) -> Result<Box<dyn Reflect>, ReflectCloneError> {
        let mut vec: SmallVec<T, N> = SmallVec::with_capacity(self.len());
        for item in self {
            vec.push(
                item.reflect_clone()?
                    .take()
                    .expect("`Reflect::reflect_clone` should return the same type"),
            );
        }
        Ok(Box::new(vec))
    }

    #[inline]
    fn to_dynamic(&self) -> Box<dyn Reflect> {
        Box::new(<Self as List>::to_dynamic_list(self))
    }
    #[inline]
    fn reflect_eq(&self, other: &dyn Reflect) -> Option<bool> {
        impls::list_eq(self, other)
    }
    #[inline]
    fn reflect_cmp(&self, other: &dyn Reflect) -> Option<core::cmp::Ordering> {
        impls::list_cmp(self, other)
    }
    #[inline]
    fn reflect_hash(&self) -> Option<u64> {
        impls::list_hash(self)
    }
    #[inline]
    fn reflect_debug(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        impls::list_debug(self, f)
    }
}

impl<T: Typed + FromReflect, const N: usize> List for SmallVec<T, N> {
    fn get(&self, index: usize) -> Option<&dyn Reflect> {
        <[T]>::get(self, index).map(Reflect::as_reflect)
    }

    fn get_mut(&mut self, index: usize) -> Option<&mut dyn Reflect> {
        <[T]>::get_mut(self, index).map(Reflect::as_mut_reflect)
    }

    fn push(&mut self, value: Box<dyn Reflect>) {
        let value = match T::take_from_reflect(value) {
            Ok(v) => v,
            Err(e) => panic! {
                "incompatible type: from {} to {}",
                e.reflect_type_path(),
                T::type_path(),
            },
        };
        Self::push(self, value);
    }

    fn try_push(&mut self, value: Box<dyn Reflect>) -> Result<(), Box<dyn Reflect>> {
        let value = T::take_from_reflect(value)?;
        Self::push(self, value);
        Ok(())
    }

    fn pop(&mut self) -> Option<Box<dyn Reflect>> {
        Self::pop(self).map(Reflect::into_boxed_reflect)
    }

    fn len(&self) -> usize {
        Self::len(self)
    }

    fn is_empty(&self) -> bool {
        Self::is_empty(self)
    }

    fn iter(&self) -> ListItemIter<'_> {
        ListItemIter::new(self)
    }

    fn drain(&mut self) -> Vec<Box<dyn Reflect>> {
        Self::drain(self, ..).map(Reflect::into_boxed_reflect).collect()
    }
}

impl<T: Typed + FromReflect, const N: usize> FromReflect for SmallVec<T, N> {
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        let ref_list = reflect.reflect_ref().as_list().ok()?;
        let mut new_list = Self::with_capacity(ref_list.len());

        for field in ref_list.iter() {
            Self::push(&mut new_list, T::from_reflect(field)?);
        }

        Some(new_list)
    }
}

impl<T: Typed + FromReflect + GetTypeMeta, const N: usize> GetTypeMeta for SmallVec<T, N> {
    fn get_type_meta() -> TypeMeta {
        let mut meta = TypeMeta::with_capacity::<Self>(2);
        meta.insert_data::<ReflectDefault>(FromType::<Self>::from_type());
        meta.insert_data::<ReflectFromReflect>(FromType::<Self>::from_type());
        meta
    }

    fn register_dependencies(registry: &mut crate::registry::TypeRegistry) {
        registry.register::<T>();
    }
}
