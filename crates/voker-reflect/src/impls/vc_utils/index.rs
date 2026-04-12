use alloc::boxed::Box;
use alloc::vec::Vec;

use voker_utils::hash::FixedHashState;
use voker_utils::index::IndexMap;
use voker_utils::index::IndexSet;
use voker_utils::index::SparseIndexMap;
use voker_utils::index::SparseIndexSet;

use crate::derive::impl_type_path;
use crate::impls::GenericTypeInfoCell;
use crate::info::{GenericInfo, Generics, TypeParamInfo};
use crate::info::{MapInfo, SetInfo, TypeInfo, TypePath, Typed};
use crate::ops::{ApplyError, Map, ReflectCloneError, Set};
use crate::registry::{FromType, GetTypeMeta, TypeMeta, TypeRegistry};
use crate::registry::{ReflectDefault, ReflectFromPtr, ReflectFromReflect};
use crate::{FromReflect, Reflect};

// -----------------------------------------------------------------------------
// IndexSet

impl_type_path!(
    ::voker_utils::index::IndexSet<T, S>
);

impl<T, S> Typed for IndexSet<T, S>
where
    T: FromReflect + Typed + Eq + ::core::hash::Hash,
    S: TypePath + ::core::hash::BuildHasher + Default + Send + Sync,
{
    fn type_info() -> &'static TypeInfo {
        static CELL: GenericTypeInfoCell = GenericTypeInfoCell::new();
        CELL.get_or_insert::<Self>(|| {
            TypeInfo::Set(SetInfo::new::<Self, T>().with_generics(Generics::from([
                GenericInfo::Type(TypeParamInfo::new::<T>("T")),
                GenericInfo::Type(TypeParamInfo::new::<S>("S").with_default::<FixedHashState>()),
            ])))
        })
    }
}

impl<T, S> Reflect for IndexSet<T, S>
where
    T: FromReflect + Typed + Eq + ::core::hash::Hash,
    S: TypePath + ::core::hash::BuildHasher + Default + Send + Sync,
{
    crate::reflection::impl_reflect_cast_fn!(Set);

    fn reflect_clone(&self) -> Result<Box<dyn Reflect>, ReflectCloneError> {
        let mut set = Self::with_capacity_and_hasher(self.len(), S::default());
        for value in self.iter() {
            let value = value
                .reflect_clone()?
                .take::<T>()
                .expect("`Reflect::reflect_clone` should return the same type");
            set.insert(value);
        }

        Ok(Box::new(set))
    }

    fn to_dynamic(&self) -> Box<dyn Reflect> {
        Box::new(<Self as Set>::to_dynamic_set(self))
    }

    #[inline]
    fn reflect_eq(&self, value: &dyn Reflect) -> Option<bool> {
        crate::impls::set_eq(self, value)
    }

    #[inline]
    fn reflect_cmp(&self, value: &dyn Reflect) -> Option<::core::cmp::Ordering> {
        crate::impls::set_cmp(self, value)
    }

    #[inline]
    fn apply(&mut self, value: &dyn Reflect) -> Result<(), ApplyError> {
        crate::impls::set_apply(self, value)
    }

    #[inline]
    fn reflect_hash(&self) -> Option<u64> {
        crate::impls::set_hash(self)
    }

    #[inline]
    fn reflect_debug(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        crate::impls::set_debug(self, f)
    }
}

impl<T, S> Set for IndexSet<T, S>
where
    T: FromReflect + Typed + Eq + ::core::hash::Hash,
    S: TypePath + ::core::hash::BuildHasher + Default + Send + Sync,
{
    fn get(&self, value: &dyn Reflect) -> Option<&dyn Reflect> {
        value
            .downcast_ref::<T>()
            .and_then(|value| Self::get(self, value))
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
        Self::drain(self, ..).map(Reflect::into_boxed_reflect).collect()
    }

    fn retain(&mut self, f: &mut dyn FnMut(&dyn Reflect) -> bool) {
        Self::retain(self, move |value| f(value));
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
            .is_some_and(|value| self.shift_remove(value))
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

impl<T, S> FromReflect for IndexSet<T, S>
where
    T: FromReflect + Typed + Eq + ::core::hash::Hash,
    S: TypePath + ::core::hash::BuildHasher + Default + Send + Sync,
{
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        let ref_set = reflect.reflect_ref().as_set().ok()?;

        let mut new_set = Self::with_capacity_and_hasher(ref_set.len(), S::default());

        for value in ref_set.iter() {
            let new_value = T::from_reflect(value)?;
            Self::insert(&mut new_set, new_value);
        }

        Some(new_set)
    }
}

impl<T, S> GetTypeMeta for IndexSet<T, S>
where
    T: FromReflect + Typed + GetTypeMeta + Eq + ::core::hash::Hash,
    S: TypePath + ::core::hash::BuildHasher + Default + Send + Sync,
{
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

// -----------------------------------------------------------------------------
// IndexMap

impl_type_path!(
    ::voker_utils::index::IndexMap<K, V, S>
);

impl<K, V, S> Typed for IndexMap<K, V, S>
where
    K: FromReflect + Typed + Eq + ::core::hash::Hash,
    V: FromReflect + Typed,
    S: TypePath + ::core::hash::BuildHasher + Default + Send + Sync,
{
    fn type_info() -> &'static TypeInfo {
        static CELL: GenericTypeInfoCell = GenericTypeInfoCell::new();
        CELL.get_or_insert::<Self>(|| {
            TypeInfo::Map(MapInfo::new::<Self, K, V>().with_generics(Generics::from([
                GenericInfo::Type(TypeParamInfo::new::<K>("K")),
                GenericInfo::Type(TypeParamInfo::new::<V>("V")),
                GenericInfo::Type(TypeParamInfo::new::<S>("S").with_default::<FixedHashState>()),
            ])))
        })
    }
}

impl<K, V, S> Reflect for IndexMap<K, V, S>
where
    K: FromReflect + Typed + Eq + ::core::hash::Hash,
    V: FromReflect + Typed,
    S: TypePath + ::core::hash::BuildHasher + Default + Send + Sync,
{
    crate::reflection::impl_reflect_cast_fn!(Map);

    fn reflect_clone(&self) -> Result<Box<dyn Reflect>, ReflectCloneError> {
        let mut map = Self::with_capacity_and_hasher(Self::len(self), S::default());
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

    fn to_dynamic(&self) -> Box<dyn Reflect> {
        Box::new(<Self as Map>::to_dynamic_map(self))
    }

    #[inline]
    fn reflect_eq(&self, value: &dyn Reflect) -> Option<bool> {
        crate::impls::map_eq(self, value)
    }

    #[inline]
    fn reflect_cmp(&self, value: &dyn Reflect) -> Option<::core::cmp::Ordering> {
        crate::impls::map_cmp(self, value)
    }

    #[inline]
    fn apply(&mut self, value: &dyn Reflect) -> Result<(), ApplyError> {
        crate::impls::map_apply(self, value)
    }

    #[inline]
    fn reflect_hash(&self) -> Option<u64> {
        crate::impls::map_hash(self)
    }

    #[inline]
    fn reflect_debug(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        crate::impls::map_debug(self, f)
    }
}

impl<K, V, S> Map for IndexMap<K, V, S>
where
    K: FromReflect + Typed + Eq + ::core::hash::Hash,
    V: FromReflect + Typed,
    S: TypePath + ::core::hash::BuildHasher + Default + Send + Sync,
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

    fn iter(&self) -> Box<dyn Iterator<Item = (&dyn Reflect, &dyn Reflect)> + '_> {
        Box::new(Self::iter(self).map(|(k, v)| (k as &dyn Reflect, v as &dyn Reflect)))
    }

    fn drain(&mut self) -> Vec<(Box<dyn Reflect>, Box<dyn Reflect>)> {
        Self::drain(self, ..)
            .map(|(key, value)| {
                (
                    Box::new(key) as Box<dyn Reflect>,
                    Box::new(value) as Box<dyn Reflect>,
                )
            })
            .collect()
    }

    fn retain(&mut self, f: &mut dyn FnMut(&dyn Reflect, &mut dyn Reflect) -> bool) {
        Self::retain(self, move |key, value| f(key, value));
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
            .and_then(|key| Self::shift_remove(self, key))
            .map(Reflect::into_boxed_reflect)
    }
}

impl<K, V, S> FromReflect for IndexMap<K, V, S>
where
    K: FromReflect + Typed + Eq + ::core::hash::Hash,
    V: FromReflect + Typed,
    S: TypePath + ::core::hash::BuildHasher + Default + Send + Sync,
{
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        let ref_map = reflect.reflect_ref().as_map().ok()?;

        let mut new_map = Self::with_capacity_and_hasher(ref_map.len(), S::default());

        for (key, value) in ref_map.iter() {
            let new_key = K::from_reflect(key)?;
            let new_value = V::from_reflect(value)?;
            Self::insert(&mut new_map, new_key, new_value);
        }

        Some(new_map)
    }
}

impl<K, V, S> GetTypeMeta for IndexMap<K, V, S>
where
    K: FromReflect + Typed + GetTypeMeta + Eq + ::core::hash::Hash,
    V: FromReflect + Typed + GetTypeMeta,
    S: TypePath + ::core::hash::BuildHasher + Default + Send + Sync,
{
    fn get_type_meta() -> TypeMeta {
        let mut type_meta = TypeMeta::with_capacity::<Self>(3);
        type_meta.insert_data::<ReflectFromPtr>(FromType::<Self>::from_type());
        type_meta.insert_data::<ReflectFromReflect>(FromType::<Self>::from_type());
        type_meta.insert_data::<ReflectDefault>(FromType::<Self>::from_type());
        type_meta
    }

    fn register_dependencies(registry: &mut TypeRegistry) {
        registry.register::<K>();
        registry.register::<V>();
    }
}

// -----------------------------------------------------------------------------
// IndexSet

impl_type_path!(::voker_utils::index::SparseIndexSet<T>);

impl<T> Typed for SparseIndexSet<T>
where
    T: FromReflect + Typed + Eq + ::core::hash::Hash,
{
    fn type_info() -> &'static TypeInfo {
        static CELL: GenericTypeInfoCell = GenericTypeInfoCell::new();
        CELL.get_or_insert::<Self>(|| {
            TypeInfo::Set(SetInfo::new::<Self, T>().with_generics(Generics::from([
                GenericInfo::Type(TypeParamInfo::new::<T>("T")),
            ])))
        })
    }
}

impl<T> Reflect for SparseIndexSet<T>
where
    T: FromReflect + Typed + Eq + ::core::hash::Hash,
{
    crate::reflection::impl_reflect_cast_fn!(Set);

    fn reflect_clone(&self) -> Result<Box<dyn Reflect>, ReflectCloneError> {
        let mut set = Self::with_capacity(self.len());
        for value in self.iter() {
            let value = value
                .reflect_clone()?
                .take::<T>()
                .expect("`Reflect::reflect_clone` should return the same type");
            set.insert(value);
        }

        Ok(Box::new(set))
    }

    fn to_dynamic(&self) -> Box<dyn Reflect> {
        Box::new(<Self as Set>::to_dynamic_set(self))
    }

    #[inline]
    fn reflect_eq(&self, value: &dyn Reflect) -> Option<bool> {
        crate::impls::set_eq(self, value)
    }

    #[inline]
    fn reflect_cmp(&self, value: &dyn Reflect) -> Option<::core::cmp::Ordering> {
        crate::impls::set_cmp(self, value)
    }

    #[inline]
    fn apply(&mut self, value: &dyn Reflect) -> Result<(), ApplyError> {
        crate::impls::set_apply(self, value)
    }

    #[inline]
    fn reflect_hash(&self) -> Option<u64> {
        crate::impls::set_hash(self)
    }

    #[inline]
    fn reflect_debug(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        crate::impls::set_debug(self, f)
    }
}

impl<T> Set for SparseIndexSet<T>
where
    T: FromReflect + Typed + Eq + ::core::hash::Hash,
{
    fn get(&self, value: &dyn Reflect) -> Option<&dyn Reflect> {
        value
            .downcast_ref::<T>()
            .and_then(|value| Self::get(self, value))
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
        Self::drain(self, ..).map(Reflect::into_boxed_reflect).collect()
    }

    fn retain(&mut self, f: &mut dyn FnMut(&dyn Reflect) -> bool) {
        Self::retain(self, move |value| f(value));
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
            .is_some_and(|value| self.shift_remove(value))
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

impl<T> FromReflect for SparseIndexSet<T>
where
    T: FromReflect + Typed + Eq + ::core::hash::Hash,
{
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        let ref_set = reflect.reflect_ref().as_set().ok()?;

        let mut new_set = Self::with_capacity(ref_set.len());

        for value in ref_set.iter() {
            let new_value = T::from_reflect(value)?;
            Self::insert(&mut new_set, new_value);
        }

        Some(new_set)
    }
}

impl<T> GetTypeMeta for SparseIndexSet<T>
where
    T: FromReflect + Typed + GetTypeMeta + Eq + ::core::hash::Hash,
{
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

// -----------------------------------------------------------------------------
// IndexMap

impl_type_path!(
    ::voker_utils::index::SparseIndexMap<K, V>
);

impl<K, V> Typed for SparseIndexMap<K, V>
where
    K: FromReflect + Typed + Eq + ::core::hash::Hash,
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

impl<K, V> Reflect for SparseIndexMap<K, V>
where
    K: FromReflect + Typed + Eq + ::core::hash::Hash,
    V: FromReflect + Typed,
{
    crate::reflection::impl_reflect_cast_fn!(Map);

    fn reflect_clone(&self) -> Result<Box<dyn Reflect>, ReflectCloneError> {
        let mut map = Self::with_capacity(Self::len(self));
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

    fn to_dynamic(&self) -> Box<dyn Reflect> {
        Box::new(<Self as Map>::to_dynamic_map(self))
    }

    #[inline]
    fn reflect_eq(&self, value: &dyn Reflect) -> Option<bool> {
        crate::impls::map_eq(self, value)
    }

    #[inline]
    fn reflect_cmp(&self, value: &dyn Reflect) -> Option<::core::cmp::Ordering> {
        crate::impls::map_cmp(self, value)
    }

    #[inline]
    fn apply(&mut self, value: &dyn Reflect) -> Result<(), ApplyError> {
        crate::impls::map_apply(self, value)
    }

    #[inline]
    fn reflect_hash(&self) -> Option<u64> {
        crate::impls::map_hash(self)
    }

    #[inline]
    fn reflect_debug(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        crate::impls::map_debug(self, f)
    }
}

impl<K, V> Map for SparseIndexMap<K, V>
where
    K: FromReflect + Typed + Eq + ::core::hash::Hash,
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

    fn iter(&self) -> Box<dyn Iterator<Item = (&dyn Reflect, &dyn Reflect)> + '_> {
        Box::new(Self::iter(self).map(|(k, v)| (k as &dyn Reflect, v as &dyn Reflect)))
    }

    fn drain(&mut self) -> Vec<(Box<dyn Reflect>, Box<dyn Reflect>)> {
        Self::drain(self, ..)
            .map(|(key, value)| {
                (
                    Box::new(key) as Box<dyn Reflect>,
                    Box::new(value) as Box<dyn Reflect>,
                )
            })
            .collect()
    }

    fn retain(&mut self, f: &mut dyn FnMut(&dyn Reflect, &mut dyn Reflect) -> bool) {
        Self::retain(self, move |key, value| f(key, value));
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
            .and_then(|key| Self::shift_remove(self, key))
            .map(Reflect::into_boxed_reflect)
    }
}

impl<K, V> FromReflect for SparseIndexMap<K, V>
where
    K: FromReflect + Typed + Eq + ::core::hash::Hash,
    V: FromReflect + Typed,
{
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        let ref_map = reflect.reflect_ref().as_map().ok()?;

        let mut new_map = Self::with_capacity(ref_map.len());

        for (key, value) in ref_map.iter() {
            let new_key = K::from_reflect(key)?;
            let new_value = V::from_reflect(value)?;
            Self::insert(&mut new_map, new_key, new_value);
        }

        Some(new_map)
    }
}

impl<K, V> GetTypeMeta for SparseIndexMap<K, V>
where
    K: FromReflect + Typed + GetTypeMeta + Eq + ::core::hash::Hash,
    V: FromReflect + Typed + GetTypeMeta,
{
    fn get_type_meta() -> TypeMeta {
        let mut type_meta = TypeMeta::with_capacity::<Self>(3);
        type_meta.insert_data::<ReflectFromPtr>(FromType::<Self>::from_type());
        type_meta.insert_data::<ReflectFromReflect>(FromType::<Self>::from_type());
        type_meta.insert_data::<ReflectDefault>(FromType::<Self>::from_type());
        type_meta
    }

    fn register_dependencies(registry: &mut TypeRegistry) {
        registry.register::<K>();
        registry.register::<V>();
    }
}
