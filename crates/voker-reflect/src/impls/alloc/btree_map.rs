use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::derive::impl_type_path;
use crate::impls::GenericTypeInfoCell;
use crate::info::{GenericInfo, Generics, MapInfo, TypeInfo, TypeParamInfo, Typed};
use crate::ops::{ApplyError, Map, ReflectCloneError};
use crate::registry::ReflectFromReflect;
use crate::registry::{FromType, GetTypeMeta, ReflectDefault, TypeMeta};
use crate::{FromReflect, Reflect};

impl_type_path!(::alloc::collections::BTreeMap<K, V>);

impl<K, V> Typed for ::alloc::collections::BTreeMap<K, V>
where
    K: FromReflect + Typed + Eq + Ord,
    V: FromReflect + Typed,
{
    fn type_info() -> &'static TypeInfo {
        static CELL: GenericTypeInfoCell = GenericTypeInfoCell::new();
        CELL.get_or_insert::<Self>(|| {
            TypeInfo::Map(MapInfo::new::<Self, K, V>().with_generics(Generics::from([
                GenericInfo::Type(TypeParamInfo::new::<K>("K")),
                GenericInfo::Type(TypeParamInfo::new::<V>("V")),
            ])))
        })
    }
}

impl<K, V> Reflect for ::alloc::collections::BTreeMap<K, V>
where
    K: FromReflect + Typed + Eq + Ord,
    V: FromReflect + Typed,
{
    crate::reflection::impl_reflect_cast_fn!(Map);

    #[inline]
    fn apply(&mut self, value: &dyn Reflect) -> Result<(), ApplyError> {
        crate::impls::map_apply(self, value)
    }

    #[inline]
    fn to_dynamic(&self) -> Box<dyn Reflect> {
        Box::new(<Self as Map>::to_dynamic_map(self))
    }

    fn reflect_clone(&self) -> Result<Box<dyn Reflect>, ReflectCloneError> {
        let mut map = Self::new();
        for (key, value) in Self::iter(self) {
            let key = key
                .reflect_clone()?
                .take::<K>()
                .expect("`Reflect::reflect_clone` should return the same type");
            let value = value
                .reflect_clone()?
                .take::<V>()
                .expect("`Reflect::reflect_clone` should return the same type");
            map.insert(key, value);
        }

        Ok(Box::new(map))
    }

    #[inline]
    fn reflect_hash(&self) -> Option<u64> {
        crate::impls::map_hash(self)
    }

    #[inline]
    fn reflect_eq(&self, value: &dyn Reflect) -> Option<bool> {
        crate::impls::map_eq(self, value)
    }

    #[inline]
    fn reflect_cmp(&self, value: &dyn Reflect) -> Option<core::cmp::Ordering> {
        crate::impls::map_cmp(self, value)
    }

    #[inline]
    fn reflect_debug(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        crate::impls::map_debug(self, f)
    }
}

impl<K, V> Map for ::alloc::collections::BTreeMap<K, V>
where
    K: FromReflect + Typed + Eq + Ord,
    V: FromReflect + Typed,
{
    fn get(&self, key: &dyn Reflect) -> Option<&dyn Reflect> {
        key.downcast_ref::<K>()
            .and_then(|key| Self::get(self, key))
            .map(Reflect::as_reflect)
    }

    fn get_mut(&mut self, key: &dyn Reflect) -> Option<&mut dyn Reflect> {
        key.downcast_ref::<K>()
            .and_then(move |key| Self::get_mut(self, key))
            .map(Reflect::as_mut_reflect)
    }

    #[inline]
    fn is_empty(&self) -> bool {
        Self::is_empty(self)
    }

    #[inline]
    fn len(&self) -> usize {
        Self::len(self)
    }

    #[inline]
    fn iter(&self) -> Box<dyn Iterator<Item = (&dyn Reflect, &dyn Reflect)> + '_> {
        Box::new(Self::iter(self).map(|(k, v)| (k as &dyn Reflect, v as &dyn Reflect)))
    }

    fn drain(&mut self) -> Vec<(Box<dyn Reflect>, Box<dyn Reflect>)> {
        let mut result = Vec::with_capacity(self.len());
        while let Some((k, v)) = self.pop_first() {
            result.push((k.into_boxed_reflect(), v.into_boxed_reflect()));
        }
        result
    }

    fn retain(&mut self, f: &mut dyn FnMut(&dyn Reflect, &mut dyn Reflect) -> bool) {
        Self::retain(self, move |k, v| f(k, v));
    }

    fn insert(
        &mut self,
        key: Box<dyn Reflect>,
        value: Box<dyn Reflect>,
    ) -> Option<Box<dyn Reflect>> {
        let key = K::take_from_reflect(key).unwrap_or_else(|key| {
            panic!(
                "Attempted to insert invalid key of type {}.",
                key.reflect_type_path()
            )
        });
        let value = V::take_from_reflect(value).unwrap_or_else(|value| {
            panic!(
                "Attempted to insert invalid value of type {}.",
                value.reflect_type_path()
            )
        });

        Self::insert(self, key, value).map(Reflect::into_boxed_reflect)
    }

    fn try_insert(
        &mut self,
        key: Box<dyn Reflect>,
        value: Box<dyn Reflect>,
    ) -> Result<Option<Box<dyn Reflect>>, (Box<dyn Reflect>, Box<dyn Reflect>)> {
        let key = match K::take_from_reflect(key) {
            Ok(k) => k,
            Err(e) => return Err((e, value)),
        };
        let value = match V::take_from_reflect(value) {
            Ok(v) => v,
            Err(e) => return Err((Box::new(key), e)),
        };
        Ok(Self::insert(self, key, value).map(Reflect::into_boxed_reflect))
    }

    fn remove(&mut self, key: &dyn Reflect) -> Option<Box<dyn Reflect>> {
        let mut from_reflect = None;
        key.downcast_ref::<K>()
            .or_else(|| {
                from_reflect = K::from_reflect(key);
                from_reflect.as_ref()
            })
            .and_then(|key| self.remove(key))
            .map(Reflect::into_boxed_reflect)
    }
}

impl<K, V> FromReflect for ::alloc::collections::BTreeMap<K, V>
where
    K: FromReflect + Typed + Eq + Ord,
    V: FromReflect + Typed,
{
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        let ref_map = reflect.reflect_ref().as_map().ok()?;

        let mut new_map = Self::new();

        for (key, value) in ref_map.iter() {
            let new_key = K::from_reflect(key)?;
            let new_value = V::from_reflect(value)?;
            new_map.insert(new_key, new_value);
        }

        Some(new_map)
    }
}

impl<K, V> GetTypeMeta for ::alloc::collections::BTreeMap<K, V>
where
    K: FromReflect + Typed + GetTypeMeta + Eq + Ord,
    V: FromReflect + Typed + GetTypeMeta,
{
    fn get_type_meta() -> TypeMeta {
        let mut type_meta = TypeMeta::with_capacity::<Self>(2);
        type_meta.insert_data::<ReflectFromReflect>(FromType::<Self>::from_type());
        type_meta.insert_data::<ReflectDefault>(FromType::<Self>::from_type());
        type_meta
    }

    fn register_dependencies(registry: &mut crate::registry::TypeRegistry) {
        registry.register::<K>();
        registry.register::<V>();
    }
}
