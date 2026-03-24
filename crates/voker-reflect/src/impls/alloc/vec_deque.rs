use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::vec::Vec;

use crate::info::{GenericInfo, Generics, ListInfo, TypeInfo, TypeParamInfo, Typed};
use crate::ops::{List, ListItemIter};
use crate::registry::{FromType, GetTypeMeta, ReflectFromPtr, TypeMeta};
use crate::registry::{ReflectDefault, ReflectFromReflect};
use crate::{FromReflect, Reflect, impls};

crate::derive::impl_type_path!(::alloc::collections::VecDeque<T>);

impl<T: Typed + FromReflect> Typed for VecDeque<T> {
    fn type_info() -> &'static TypeInfo {
        static CELL: impls::GenericTypeInfoCell = impls::GenericTypeInfoCell::new();
        CELL.get_or_insert::<Self>(|| {
            TypeInfo::List(ListInfo::new::<Self, T>().with_generics(Generics::from([
                GenericInfo::Type(TypeParamInfo::new::<T>("T")),
            ])))
        })
    }
}

impl<T: Typed + FromReflect> Reflect for VecDeque<T> {
    crate::reflection::impl_reflect_cast_fn!(List);
    #[inline]
    fn apply(&mut self, value: &dyn Reflect) -> Result<(), crate::ops::ApplyError> {
        impls::list_apply(self, value)
    }

    fn reflect_clone(&self) -> Result<Box<dyn Reflect>, crate::ops::ReflectCloneError> {
        let mut vec: VecDeque<T> = VecDeque::with_capacity(self.len());
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

impl<T: Typed + FromReflect> List for VecDeque<T> {
    fn get(&self, index: usize) -> Option<&dyn Reflect> {
        Self::get(self, index).map(Reflect::as_reflect)
    }

    fn get_mut(&mut self, index: usize) -> Option<&mut dyn Reflect> {
        Self::get_mut(self, index).map(Reflect::as_reflect_mut)
    }

    fn insert(&mut self, index: usize, element: Box<dyn Reflect>) {
        let element = match T::take_from_reflect(element) {
            Ok(v) => v,
            Err(e) => panic! {
                "incompatible type: from {} to {}",
                e.reflect_type_path(),
                T::type_path(),
            },
        };
        Self::insert(self, index, element);
    }

    fn remove(&mut self, index: usize) -> Box<dyn Reflect> {
        Box::new(Self::remove(self, index).expect("index out of bound"))
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
        Self::push_back(self, value);
    }

    fn try_push(&mut self, value: Box<dyn Reflect>) -> Result<(), Box<dyn Reflect>> {
        let value = T::take_from_reflect(value)?;
        Self::push_back(self, value);
        Ok(())
    }

    fn pop(&mut self) -> Option<Box<dyn Reflect>> {
        Self::pop_back(self).map(Reflect::into_boxed_reflect)
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
        self.drain(..).map(Reflect::into_boxed_reflect).collect()
    }
}

impl<T: Typed + FromReflect + GetTypeMeta> GetTypeMeta for VecDeque<T> {
    fn get_type_meta() -> TypeMeta {
        let mut meta = TypeMeta::with_capacity::<Self>(3);
        meta.insert_trait::<ReflectDefault>(FromType::<Self>::from_type());
        meta.insert_trait::<ReflectFromPtr>(FromType::<Self>::from_type());
        meta.insert_trait::<ReflectFromReflect>(FromType::<Self>::from_type());
        meta
    }

    fn register_dependencies(registry: &mut crate::registry::TypeRegistry) {
        registry.register::<T>();
    }
}

impl<T: Typed + FromReflect> FromReflect for VecDeque<T> {
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        let ref_list = reflect.reflect_ref().as_list().ok()?;
        let mut new_list = Self::with_capacity(ref_list.len());

        for field in ref_list.iter() {
            Self::push_back(&mut new_list, T::from_reflect(field)?);
        }

        Some(new_list)
    }
}
