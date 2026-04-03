use alloc::boxed::Box;
use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use crate::derive::impl_type_path;
use crate::impls::GenericTypeInfoCell;
use crate::info::{GenericInfo, Generics, SetInfo, TypeInfo, TypeParamInfo, Typed};
use crate::ops::{ApplyError, ReflectCloneError, Set};
use crate::registry::{FromType, GetTypeMeta, TypeMeta, TypeRegistry};
use crate::registry::{ReflectDefault, ReflectFromPtr, ReflectFromReflect};
use crate::{FromReflect, Reflect};

impl_type_path!(::alloc::collections::BTreeSet<T>);

impl<T: FromReflect + Typed + Ord + Eq> Typed for BTreeSet<T> {
    fn type_info() -> &'static TypeInfo {
        static CELL: GenericTypeInfoCell = GenericTypeInfoCell::new();
        CELL.get_or_insert::<Self>(|| {
            TypeInfo::Set(SetInfo::new::<Self, T>().with_generics(Generics::from([
                GenericInfo::Type(TypeParamInfo::new::<T>("T")),
            ])))
        })
    }
}

impl<T: FromReflect + Typed + Ord + Eq> Reflect for BTreeSet<T> {
    crate::reflection::impl_reflect_cast_fn!(Set);

    #[inline]
    fn apply(&mut self, value: &dyn Reflect) -> Result<(), ApplyError> {
        crate::impls::set_apply(self, value)
    }

    #[inline]
    fn to_dynamic(&self) -> Box<dyn Reflect> {
        Box::new(<Self as Set>::to_dynamic_set(self))
    }

    fn reflect_clone(&self) -> Result<Box<dyn Reflect>, ReflectCloneError> {
        let mut set = Self::new();
        for value in Self::iter(self) {
            let value = value
                .reflect_clone()?
                .take::<T>()
                .expect("`Reflect::reflect_clone` should return the same type");
            set.insert(value);
        }
        Ok(Box::new(set))
    }

    #[inline]
    fn reflect_hash(&self) -> Option<u64> {
        crate::impls::set_hash(self)
    }

    #[inline]
    fn reflect_eq(&self, value: &dyn Reflect) -> Option<bool> {
        crate::impls::set_eq(self, value)
    }

    #[inline]
    fn reflect_cmp(&self, value: &dyn Reflect) -> Option<core::cmp::Ordering> {
        crate::impls::set_cmp(self, value)
    }

    #[inline]
    fn reflect_debug(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        crate::impls::set_debug(self, f)
    }
}

impl<T: FromReflect + Typed + Ord + Eq> Set for BTreeSet<T> {
    fn get(&self, value: &dyn Reflect) -> Option<&dyn Reflect> {
        value
            .downcast_ref::<T>()
            .and_then(|key| Self::get(self, key))
            .map(Reflect::as_reflect)
    }

    #[inline]
    fn is_empty(&self) -> bool {
        Self::is_empty(self)
    }

    #[inline]
    fn len(&self) -> usize {
        Self::len(self)
    }

    fn iter(&self) -> Box<dyn Iterator<Item = &dyn Reflect> + '_> {
        Box::new(Self::iter(self).map(Reflect::as_reflect))
    }

    fn drain(&mut self) -> Vec<Box<dyn Reflect>> {
        let mut result = Vec::with_capacity(self.len());
        while let Some(v) = self.pop_first() {
            result.push(v.into_boxed_reflect());
        }
        result
    }

    fn retain(&mut self, f: &mut dyn FnMut(&dyn Reflect) -> bool) {
        Self::retain(self, |v| f(v));
    }

    fn insert(&mut self, value: Box<dyn Reflect>) -> bool {
        let value = T::take_from_reflect(value).unwrap_or_else(|value| {
            panic!(
                "Attempted to insert invalid value of type {}.",
                value.reflect_type_path()
            )
        });
        Self::insert(self, value)
    }

    fn try_insert(&mut self, value: Box<dyn Reflect>) -> Result<bool, Box<dyn Reflect>> {
        let value = T::take_from_reflect(value)?;
        Ok(Self::insert(self, value))
    }

    fn remove(&mut self, value: &dyn Reflect) -> bool {
        let mut from_reflect = None;
        value
            .downcast_ref::<T>()
            .or_else(|| {
                from_reflect = T::from_reflect(value);
                from_reflect.as_ref()
            })
            .is_some_and(|value| self.remove(value))
    }

    fn contains(&self, value: &dyn Reflect) -> bool {
        let mut from_reflect = None;
        value
            .downcast_ref::<T>()
            .or_else(|| {
                from_reflect = T::from_reflect(value);
                from_reflect.as_ref()
            })
            .is_some_and(|value| self.contains(value))
    }
}

impl<T: FromReflect + Typed + Ord + Eq> FromReflect for BTreeSet<T> {
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        let ref_set = reflect.reflect_ref().as_set().ok()?;

        let mut new_set = Self::new();

        for value in ref_set.iter() {
            let new_value = T::from_reflect(value)?;
            new_set.insert(new_value);
        }

        Some(new_set)
    }
}

impl<T: FromReflect + Typed + Ord + Eq + GetTypeMeta> GetTypeMeta for BTreeSet<T> {
    fn get_type_meta() -> TypeMeta {
        let mut type_meta = TypeMeta::with_capacity::<Self>(3);
        type_meta.insert_data::<ReflectFromPtr>(FromType::<Self>::from_type());
        type_meta.insert_data::<ReflectFromReflect>(FromType::<Self>::from_type());
        type_meta.insert_data::<ReflectDefault>(FromType::<Self>::from_type());
        type_meta
    }

    fn register_dependencies(registry: &mut TypeRegistry) {
        registry.register::<T>();
    }
}
