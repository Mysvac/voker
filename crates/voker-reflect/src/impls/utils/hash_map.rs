// -----------------------------------------------------------------------------
// For normal HashMap

macro_rules! impl_reflect_for_hashmap {
    ($ty:path $(, $default_state:path)? $(,)?) => {
        impl<K, V, S> $crate::info::Typed for $ty
        where
            K: $crate::FromReflect + $crate::info::Typed + Eq + ::core::hash::Hash,
            V: $crate::FromReflect + $crate::info::Typed,
            S: $crate::info::TypePath + ::core::hash::BuildHasher + Default + Send + Sync,
        {
            fn type_info() -> &'static $crate::info::TypeInfo {
                static CELL: $crate::impls::GenericTypeInfoCell = $crate::impls::GenericTypeInfoCell::new();
                CELL.get_or_insert::<Self>(|| {
                    $crate::info::TypeInfo::Map(
                        $crate::info::MapInfo::new::<Self, K, V>().with_generics($crate::info::Generics::from([
                            $crate::info::GenericInfo::Type($crate::info::TypeParamInfo::new::<K>("K")),
                            $crate::info::GenericInfo::Type($crate::info::TypeParamInfo::new::<V>("V")),
                            $crate::info::GenericInfo::Type(
                                $crate::info::TypeParamInfo::new::<S>("S")$(.with_default::<$default_state>())?
                            ),
                        ])),
                    )
                })
            }
        }

        impl<K, V, S> $crate::Reflect for $ty
        where
            K: $crate::FromReflect + $crate::info::Typed + Eq + ::core::hash::Hash,
            V: $crate::FromReflect + $crate::info::Typed,
            S: $crate::info::TypePath + ::core::hash::BuildHasher + Default + Send + Sync,
        {
            $crate::reflection::impl_reflect_cast_fn!(Map);

            fn reflect_clone(&self) -> Result<::alloc::boxed::Box<dyn $crate::Reflect>, $crate::ops::ReflectCloneError> {
                let mut map = Self::with_capacity_and_hasher(Self::len(self), S::default());
                for (key, value) in Self::iter(self) {
                    let key = key.reflect_clone()?.take::<K>().expect("`Reflect::reflect_clone` should return the same type");
                    let value = value.reflect_clone()?.take::<V>().expect("`Reflect::reflect_clone` should return the same type");
                    map.insert(key, value);
                }

                Ok(::alloc::boxed::Box::new(map))
            }

            fn to_dynamic(&self) -> ::alloc::boxed::Box<dyn $crate::Reflect> {
                ::alloc::boxed::Box::new(<Self as $crate::ops::Map>::to_dynamic_map(self))
            }

            #[inline]
            fn reflect_eq(&self, value: &dyn $crate::Reflect) -> Option<bool> {
                $crate::impls::map_eq(self, value)
            }

            #[inline]
            fn reflect_cmp(&self, value: &dyn $crate::Reflect) -> Option<::core::cmp::Ordering> {
                $crate::impls::map_cmp(self, value)
            }

            #[inline]
            fn apply(&mut self, value: &dyn $crate::Reflect) -> Result<(), $crate::ops::ApplyError> {
                $crate::impls::map_apply(self, value)
            }

            #[inline]
            fn reflect_hash(&self) -> Option<u64> {
                $crate::impls::map_hash(self)
            }

            #[inline]
            fn reflect_debug(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                $crate::impls::map_debug(self, f)
            }
        }

        impl<K, V, S> $crate::ops::Map for $ty
        where
            K: $crate::FromReflect + $crate::info::Typed + Eq + ::core::hash::Hash,
            V: $crate::FromReflect + $crate::info::Typed,
            S: $crate::info::TypePath + ::core::hash::BuildHasher + Default + Send + Sync,
        {
            fn get(&self, key: &dyn $crate::Reflect) -> Option<&dyn $crate::Reflect> {
                key.downcast_ref::<K>()
                    .and_then(|key| Self::get(self, key))
                    .map($crate::Reflect::as_reflect)
            }

            fn get_mut(&mut self, key: &dyn $crate::Reflect) -> Option<&mut dyn $crate::Reflect> {
                key.downcast_ref::<K>()
                    .and_then(move |key| Self::get_mut(self, key))
                    .map($crate::Reflect::as_mut_reflect)
            }

            #[inline]
            fn is_empty(&self) -> bool {
                Self::is_empty(self)
            }

            #[inline]
            fn len(&self) -> usize {
                Self::len(self)
            }

            fn iter(&self) -> ::alloc::boxed::Box<dyn Iterator<Item = (&dyn $crate::Reflect, &dyn $crate::Reflect)> + '_> {
                ::alloc::boxed::Box::new(Self::iter(self).map(|(k, v)| (k as &dyn $crate::Reflect, v as &dyn $crate::Reflect)))
            }

            fn drain(&mut self) -> ::alloc::vec::Vec<(::alloc::boxed::Box<dyn $crate::Reflect>, ::alloc::boxed::Box<dyn $crate::Reflect>)> {
                Self::drain(self)
                    .map(|(key, value)| (
                        ::alloc::boxed::Box::new(key) as ::alloc::boxed::Box<dyn $crate::Reflect>,
                        ::alloc::boxed::Box::new(value) as ::alloc::boxed::Box<dyn $crate::Reflect>,
                    ))
                    .collect()
            }

            fn retain(&mut self, f: &mut dyn FnMut(&dyn $crate::Reflect, &mut dyn $crate::Reflect) -> bool) {
                Self::retain(self, move |key, value| f(key, value));
            }

            fn insert(
                &mut self,
                key: ::alloc::boxed::Box<dyn $crate::Reflect>,
                value: ::alloc::boxed::Box<dyn $crate::Reflect>,
            ) -> Option<::alloc::boxed::Box<dyn $crate::Reflect>> {
                let key = K::take_from_reflect(key).unwrap_or_else(|key| panic!(
                    "Attempted to insert invalid key of type {}.",
                    key.reflect_type_path()
                ));
                let value = V::take_from_reflect(value).unwrap_or_else(|value| panic!(
                    "Attempted to insert invalid value of type {}.",
                    value.reflect_type_path()
                ));
                Self::insert(self, key, value).map($crate::Reflect::into_boxed_reflect)
            }

            fn try_insert(
                &mut self,
                key: ::alloc::boxed::Box<dyn $crate::Reflect>,
                value: ::alloc::boxed::Box<dyn $crate::Reflect>,
            ) -> Result<Option<::alloc::boxed::Box<dyn $crate::Reflect>>, (::alloc::boxed::Box<dyn $crate::Reflect>, ::alloc::boxed::Box<dyn $crate::Reflect>)> {
                let key = match K::take_from_reflect(key) {
                    Ok(k) => k,
                    Err(e) => return Err((e, value)),
                };
                let value = match V::take_from_reflect(value) {
                    Ok(v) => v,
                    Err(e) => return Err((::alloc::boxed::Box::new(key), e)),
                };
                Ok(Self::insert(self, key, value).map($crate::Reflect::into_boxed_reflect))
            }

            fn remove(&mut self, key: &dyn $crate::Reflect) -> Option<::alloc::boxed::Box<dyn $crate::Reflect>> {
                let mut from_reflect = None;
                key.downcast_ref::<K>()
                    .or_else(|| {
                        from_reflect = K::from_reflect(key);
                        from_reflect.as_ref()
                    })
                    .and_then(|key| Self::remove(self, key))
                    .map($crate::Reflect::into_boxed_reflect)
            }
        }


        impl<K, V, S> $crate::FromReflect for $ty
        where
            K: $crate::FromReflect + $crate::info::Typed + Eq + ::core::hash::Hash,
            V: $crate::FromReflect + $crate::info::Typed,
            S: $crate::info::TypePath + ::core::hash::BuildHasher + Default + Send + Sync,
        {
            fn from_reflect(reflect: &dyn $crate::Reflect) -> Option<Self> {
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

        impl<K, V, S> $crate::registry::GetTypeMeta for $ty
        where
            K: $crate::FromReflect + $crate::info::Typed + $crate::registry::GetTypeMeta + Eq + ::core::hash::Hash,
            V: $crate::FromReflect + $crate::info::Typed + $crate::registry::GetTypeMeta,
            S: $crate::info::TypePath + ::core::hash::BuildHasher + Default + Send + Sync,
        {
            fn get_type_meta() -> $crate::registry::TypeMeta {
                let mut type_meta = $crate::registry::TypeMeta::with_capacity::<Self>(2);
                type_meta.insert_data::<$crate::registry::ReflectFromReflect>($crate::registry::FromType::<Self>::from_type());
                type_meta.insert_data::<$crate::registry::ReflectDefault>($crate::registry::FromType::<Self>::from_type());
                type_meta
            }

            fn register_dependencies(registry: &mut $crate::registry::TypeRegistry) {
                registry.register::<K>();
                registry.register::<V>();
            }
        }
    };
}

pub(crate) use impl_reflect_for_hashmap;

// -----------------------------------------------------------------------------
// For NoopHashMap

macro_rules! impl_reflect_for_fixedhashmap {
    ($ty:path) => {
        impl<K, V> $crate::info::Typed for $ty
        where
            K: $crate::FromReflect + $crate::info::Typed + Eq + ::core::hash::Hash,
            V: $crate::FromReflect + $crate::info::Typed,
        {
            fn type_info() -> &'static $crate::info::TypeInfo {
                static CELL: $crate::impls::GenericTypeInfoCell =
                    $crate::impls::GenericTypeInfoCell::new();
                CELL.get_or_insert::<Self>(|| {
                    $crate::info::TypeInfo::Map(
                        $crate::info::MapInfo::new::<Self, K, V>().with_generics(
                            $crate::info::Generics::from([
                                $crate::info::GenericInfo::Type(
                                    $crate::info::TypeParamInfo::new::<K>("K"),
                                ),
                                $crate::info::GenericInfo::Type(
                                    $crate::info::TypeParamInfo::new::<V>("V"),
                                ),
                            ]),
                        ),
                    )
                })
            }
        }

        impl<K, V> $crate::Reflect for $ty
        where
            K: $crate::FromReflect + $crate::info::Typed + Eq + ::core::hash::Hash,
            V: $crate::FromReflect + $crate::info::Typed,
        {
            $crate::reflection::impl_reflect_cast_fn!(Map);

            fn reflect_clone(
                &self,
            ) -> Result<::alloc::boxed::Box<dyn $crate::Reflect>, $crate::ops::ReflectCloneError>
            {
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

                Ok(::alloc::boxed::Box::new(map))
            }

            fn to_dynamic(&self) -> ::alloc::boxed::Box<dyn $crate::Reflect> {
                ::alloc::boxed::Box::new(<Self as $crate::ops::Map>::to_dynamic_map(self))
            }

            #[inline]
            fn reflect_eq(&self, value: &dyn $crate::Reflect) -> Option<bool> {
                $crate::impls::map_eq(self, value)
            }

            #[inline]
            fn reflect_cmp(&self, value: &dyn $crate::Reflect) -> Option<::core::cmp::Ordering> {
                $crate::impls::map_cmp(self, value)
            }

            #[inline]
            fn apply(
                &mut self,
                value: &dyn $crate::Reflect,
            ) -> Result<(), $crate::ops::ApplyError> {
                $crate::impls::map_apply(self, value)
            }

            #[inline]
            fn reflect_hash(&self) -> Option<u64> {
                $crate::impls::map_hash(self)
            }

            #[inline]
            fn reflect_debug(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                $crate::impls::map_debug(self, f)
            }
        }

        impl<K, V> $crate::ops::Map for $ty
        where
            K: $crate::FromReflect + $crate::info::Typed + Eq + ::core::hash::Hash,
            V: $crate::FromReflect + $crate::info::Typed,
        {
            fn get(&self, key: &dyn $crate::Reflect) -> Option<&dyn $crate::Reflect> {
                key.downcast_ref::<K>()
                    .and_then(|key| Self::get(self, key))
                    .map($crate::Reflect::as_reflect)
            }

            fn get_mut(&mut self, key: &dyn $crate::Reflect) -> Option<&mut dyn $crate::Reflect> {
                key.downcast_ref::<K>()
                    .and_then(move |key| Self::get_mut(self, key))
                    .map($crate::Reflect::as_mut_reflect)
            }

            #[inline]
            fn is_empty(&self) -> bool {
                Self::is_empty(self)
            }

            #[inline]
            fn len(&self) -> usize {
                Self::len(self)
            }

            fn iter(
                &self,
            ) -> ::alloc::boxed::Box<
                dyn Iterator<Item = (&dyn $crate::Reflect, &dyn $crate::Reflect)> + '_,
            > {
                ::alloc::boxed::Box::new(
                    Self::iter(self)
                        .map(|(k, v)| (k as &dyn $crate::Reflect, v as &dyn $crate::Reflect)),
                )
            }

            fn drain(
                &mut self,
            ) -> ::alloc::vec::Vec<(
                ::alloc::boxed::Box<dyn $crate::Reflect>,
                ::alloc::boxed::Box<dyn $crate::Reflect>,
            )> {
                Self::drain(self)
                    .map(|(key, value)| {
                        (
                            ::alloc::boxed::Box::new(key)
                                as ::alloc::boxed::Box<dyn $crate::Reflect>,
                            ::alloc::boxed::Box::new(value)
                                as ::alloc::boxed::Box<dyn $crate::Reflect>,
                        )
                    })
                    .collect()
            }

            fn retain(
                &mut self,
                f: &mut dyn FnMut(&dyn $crate::Reflect, &mut dyn $crate::Reflect) -> bool,
            ) {
                Self::retain(self, move |key, value| f(key, value));
            }

            fn insert(
                &mut self,
                key: ::alloc::boxed::Box<dyn $crate::Reflect>,
                value: ::alloc::boxed::Box<dyn $crate::Reflect>,
            ) -> Option<::alloc::boxed::Box<dyn $crate::Reflect>> {
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
                Self::insert(self, key, value).map($crate::Reflect::into_boxed_reflect)
            }

            fn try_insert(
                &mut self,
                key: ::alloc::boxed::Box<dyn $crate::Reflect>,
                value: ::alloc::boxed::Box<dyn $crate::Reflect>,
            ) -> Result<
                Option<::alloc::boxed::Box<dyn $crate::Reflect>>,
                (
                    ::alloc::boxed::Box<dyn $crate::Reflect>,
                    ::alloc::boxed::Box<dyn $crate::Reflect>,
                ),
            > {
                let key = match K::take_from_reflect(key) {
                    Ok(k) => k,
                    Err(e) => return Err((e, value)),
                };
                let value = match V::take_from_reflect(value) {
                    Ok(v) => v,
                    Err(e) => return Err((::alloc::boxed::Box::new(key), e)),
                };
                Ok(Self::insert(self, key, value).map($crate::Reflect::into_boxed_reflect))
            }

            fn remove(
                &mut self,
                key: &dyn $crate::Reflect,
            ) -> Option<::alloc::boxed::Box<dyn $crate::Reflect>> {
                let mut from_reflect = None;
                key.downcast_ref::<K>()
                    .or_else(|| {
                        from_reflect = K::from_reflect(key);
                        from_reflect.as_ref()
                    })
                    .and_then(|key| Self::remove(self, key))
                    .map($crate::Reflect::into_boxed_reflect)
            }
        }

        impl<K, V> $crate::FromReflect for $ty
        where
            K: $crate::FromReflect + $crate::info::Typed + Eq + ::core::hash::Hash,
            V: $crate::FromReflect + $crate::info::Typed,
        {
            fn from_reflect(reflect: &dyn $crate::Reflect) -> Option<Self> {
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

        impl<K, V> $crate::registry::GetTypeMeta for $ty
        where
            K: $crate::FromReflect
                + $crate::info::Typed
                + $crate::registry::GetTypeMeta
                + Eq
                + ::core::hash::Hash,
            V: $crate::FromReflect + $crate::info::Typed + $crate::registry::GetTypeMeta,
        {
            fn get_type_meta() -> $crate::registry::TypeMeta {
                let mut type_meta = $crate::registry::TypeMeta::with_capacity::<Self>(2);
                type_meta.insert_data::<$crate::registry::ReflectFromReflect>(
                    $crate::registry::FromType::<Self>::from_type(),
                );
                type_meta.insert_data::<$crate::registry::ReflectDefault>(
                    $crate::registry::FromType::<Self>::from_type(),
                );
                type_meta
            }

            fn register_dependencies(registry: &mut $crate::registry::TypeRegistry) {
                registry.register::<K>();
                registry.register::<V>();
            }
        }
    };
}

pub(crate) use impl_reflect_for_fixedhashmap;
