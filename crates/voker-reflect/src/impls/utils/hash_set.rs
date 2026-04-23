// -----------------------------------------------------------------------------
// For normal HashSet

macro_rules! impl_reflect_for_hashset {
    ($ty:path $(, $default_state:path)? $(,)?) => {
        impl<T, S> $crate::info::Typed for $ty
        where
            T: $crate::FromReflect + $crate::info::Typed + Eq + ::core::hash::Hash,
            S: $crate::info::TypePath + ::core::hash::BuildHasher + Default + Send + Sync,
        {
            fn type_info() -> &'static $crate::info::TypeInfo {
                static CELL: $crate::impls::GenericTypeInfoCell = $crate::impls::GenericTypeInfoCell::new();
                CELL.get_or_insert::<Self>(|| {
                    $crate::info::TypeInfo::Set(
                        $crate::info::SetInfo::new::<Self, T>().with_generics($crate::info::Generics::from([
                            $crate::info::GenericInfo::Type($crate::info::TypeParamInfo::new::<T>("T")),
                            $crate::info::GenericInfo::Type(
                                $crate::info::TypeParamInfo::new::<S>("S")$(.with_default::<$default_state>())?
                            ),
                        ]))
                    )
                })
            }
        }

        impl<T, S> $crate::Reflect for $ty
        where
            T: $crate::FromReflect + $crate::info::Typed + Eq + ::core::hash::Hash,
            S: $crate::info::TypePath + ::core::hash::BuildHasher + Default + Send + Sync,
        {
            $crate::reflection::impl_reflect_cast_fn!(Set);

            fn reflect_clone(&self) -> Result<::alloc::boxed::Box<dyn $crate::Reflect>, $crate::ops::ReflectCloneError> {
                let mut set = Self::with_capacity_and_hasher(self.len(), S::default());
                for value in self.iter() {
                    let value = value.reflect_clone()?.take::<T>().expect("`Reflect::reflect_clone` should return the same type");
                    set.insert(value);
                }

                Ok(::alloc::boxed::Box::new(set))
            }

            fn to_dynamic(&self) -> ::alloc::boxed::Box<dyn $crate::Reflect> {
                ::alloc::boxed::Box::new(<Self as $crate::ops::Set>::to_dynamic_set(self))
            }

            #[inline]
            fn reflect_eq(&self, value: &dyn $crate::Reflect) -> Option<bool> {
                $crate::impls::set_eq(self, value)
            }

            #[inline]
            fn reflect_cmp(&self, value: &dyn $crate::Reflect) -> Option<::core::cmp::Ordering> {
                $crate::impls::set_cmp(self, value)
            }

            #[inline]
            fn apply(&mut self, value: &dyn $crate::Reflect) -> Result<(), $crate::ops::ApplyError> {
                $crate::impls::set_apply(self, value)
            }

            #[inline]
            fn reflect_hash(&self) -> Option<u64> {
                $crate::impls::set_hash(self)
            }

            #[inline]
            fn reflect_debug(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                $crate::impls::set_debug(self, f)
            }
        }

        impl<T, S> $crate::ops::Set for $ty
        where
            T: $crate::FromReflect + $crate::info::Typed + Eq + ::core::hash::Hash,
            S: $crate::info::TypePath + ::core::hash::BuildHasher + Default + Send + Sync,
        {
            fn get(&self, value: &dyn $crate::Reflect) -> Option<&dyn $crate::Reflect> {
                value
                    .downcast_ref::<T>()
                    .and_then(|value| Self::get(self, value))
                    .map($crate::Reflect::as_reflect)
            }

            #[inline]
            fn is_empty(&self) -> bool {
                Self::is_empty(self)
            }

            #[inline]
            fn len(&self) -> usize {
                Self::len(self)
            }

            fn iter(&self) -> ::alloc::boxed::Box<dyn Iterator<Item = &dyn $crate::Reflect> + '_> {
                ::alloc::boxed::Box::new(Self::iter(self).map($crate::Reflect::as_reflect))
            }

            fn drain(&mut self) -> ::alloc::vec::Vec<::alloc::boxed::Box<dyn $crate::Reflect>> {
                Self::drain(self)
                    .map($crate::Reflect::into_boxed_reflect)
                    .collect()
            }

            fn retain(&mut self, f: &mut dyn FnMut(&dyn $crate::Reflect) -> bool) {
                Self::retain(self, move |value| f(value));
            }

            fn insert(&mut self, value: ::alloc::boxed::Box<dyn $crate::Reflect>) -> bool {
                let value = T::take_from_reflect(value).unwrap_or_else(|value| panic!(
                    "Attempted to insert invalid value of type {}.",
                    value.reflect_type_path()
                ));
                Self::insert(self, value)
            }

            fn try_insert(
                &mut self,
                value: ::alloc::boxed::Box<dyn $crate::Reflect>,
            ) -> Result<bool, ::alloc::boxed::Box<dyn $crate::Reflect>> {
                let value = T::take_from_reflect(value)?;
                Ok(Self::insert(self, value))
            }


            fn remove(&mut self, value: &dyn $crate::Reflect) -> bool {
                let mut from_reflect = None;
                value
                    .downcast_ref::<T>()
                    .or_else(|| {
                        from_reflect = T::from_reflect(value);
                        from_reflect.as_ref()
                    })
                    .is_some_and(|value| self.remove(value))
            }

            fn contains(&self, value: &dyn $crate::Reflect) -> bool {
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

        impl<T, S> $crate::FromReflect for $ty
        where
            T: $crate::FromReflect + $crate::info::Typed + Eq + ::core::hash::Hash,
            S: $crate::info::TypePath + ::core::hash::BuildHasher + Default + Send + Sync,
        {
            fn from_reflect(reflect: &dyn $crate::Reflect) -> Option<Self> {
                let ref_set = reflect.reflect_ref().as_set().ok()?;

                let mut new_set = Self::with_capacity_and_hasher(ref_set.len(), S::default());

                for value in ref_set.iter() {
                    let new_value = T::from_reflect(value)?;
                    Self::insert(&mut new_set, new_value);
                }

                Some(new_set)
            }
        }

        impl<T, S> $crate::registry::GetTypeMeta for $ty
        where
            T: $crate::FromReflect + $crate::info::Typed + $crate::registry::GetTypeMeta + Eq + ::core::hash::Hash,
            S: $crate::info::TypePath + ::core::hash::BuildHasher + Default + Send + Sync,
        {
            fn get_type_meta() -> $crate::registry::TypeMeta {
                let mut type_meta = $crate::registry::TypeMeta::with_capacity::<Self>(2);
                type_meta.insert_data::<$crate::registry::ReflectFromReflect>($crate::registry::FromType::<Self>::from_type());
                type_meta.insert_data::<$crate::registry::ReflectDefault>($crate::registry::FromType::<Self>::from_type());
                type_meta
            }

            fn register_dependencies(registry: &mut $crate::registry::TypeRegistry) {
                registry.register::<T>();
            }
        }
    };
}

pub(crate) use impl_reflect_for_hashset;

// -----------------------------------------------------------------------------
// For NoopHashSet

macro_rules! impl_reflect_for_fixedhashset {
    ($ty:path) => {
        impl<T> $crate::info::Typed for $ty
        where
            T: $crate::FromReflect + $crate::info::Typed + Eq + ::core::hash::Hash,
        {
            fn type_info() -> &'static $crate::info::TypeInfo {
                static CELL: $crate::impls::GenericTypeInfoCell =
                    $crate::impls::GenericTypeInfoCell::new();
                CELL.get_or_insert::<Self>(|| {
                    $crate::info::TypeInfo::Set(
                        $crate::info::SetInfo::new::<Self, T>().with_generics(
                            $crate::info::Generics::from([$crate::info::GenericInfo::Type(
                                $crate::info::TypeParamInfo::new::<T>("T"),
                            )]),
                        ),
                    )
                })
            }
        }

        impl<T> $crate::Reflect for $ty
        where
            T: $crate::FromReflect + $crate::info::Typed + Eq + ::core::hash::Hash,
        {
            $crate::reflection::impl_reflect_cast_fn!(Set);

            fn reflect_clone(
                &self,
            ) -> Result<::alloc::boxed::Box<dyn $crate::Reflect>, $crate::ops::ReflectCloneError>
            {
                let mut set = Self::with_capacity(self.len());
                for value in self.iter() {
                    let value = value
                        .reflect_clone()?
                        .take::<T>()
                        .expect("`Reflect::reflect_clone` should return the same type");
                    set.insert(value);
                }

                Ok(::alloc::boxed::Box::new(set))
            }

            fn to_dynamic(&self) -> ::alloc::boxed::Box<dyn $crate::Reflect> {
                ::alloc::boxed::Box::new(<Self as $crate::ops::Set>::to_dynamic_set(self))
            }

            #[inline]
            fn reflect_eq(&self, value: &dyn $crate::Reflect) -> Option<bool> {
                $crate::impls::set_eq(self, value)
            }

            #[inline]
            fn reflect_cmp(&self, value: &dyn $crate::Reflect) -> Option<::core::cmp::Ordering> {
                $crate::impls::set_cmp(self, value)
            }

            #[inline]
            fn apply(
                &mut self,
                value: &dyn $crate::Reflect,
            ) -> Result<(), $crate::ops::ApplyError> {
                $crate::impls::set_apply(self, value)
            }

            #[inline]
            fn reflect_hash(&self) -> Option<u64> {
                $crate::impls::set_hash(self)
            }

            #[inline]
            fn reflect_debug(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                $crate::impls::set_debug(self, f)
            }
        }

        impl<T> $crate::ops::Set for $ty
        where
            T: $crate::FromReflect + $crate::info::Typed + Eq + ::core::hash::Hash,
        {
            fn get(&self, value: &dyn $crate::Reflect) -> Option<&dyn $crate::Reflect> {
                value
                    .downcast_ref::<T>()
                    .and_then(|value| Self::get(self, value))
                    .map($crate::Reflect::as_reflect)
            }

            #[inline]
            fn is_empty(&self) -> bool {
                Self::is_empty(self)
            }

            #[inline]
            fn len(&self) -> usize {
                Self::len(self)
            }

            fn iter(&self) -> ::alloc::boxed::Box<dyn Iterator<Item = &dyn $crate::Reflect> + '_> {
                ::alloc::boxed::Box::new(Self::iter(self).map($crate::Reflect::as_reflect))
            }

            fn drain(&mut self) -> ::alloc::vec::Vec<::alloc::boxed::Box<dyn $crate::Reflect>> {
                self.drain().map($crate::Reflect::into_boxed_reflect).collect()
            }

            fn retain(&mut self, f: &mut dyn FnMut(&dyn $crate::Reflect) -> bool) {
                Self::retain(self, move |value| f(value));
            }

            fn insert(&mut self, value: ::alloc::boxed::Box<dyn $crate::Reflect>) -> bool {
                let value = T::take_from_reflect(value).unwrap_or_else(|value| {
                    panic!(
                        "Attempted to insert invalid value of type {}.",
                        value.reflect_type_path()
                    )
                });
                Self::insert(self, value)
            }

            fn try_insert(
                &mut self,
                value: ::alloc::boxed::Box<dyn $crate::Reflect>,
            ) -> Result<bool, ::alloc::boxed::Box<dyn $crate::Reflect>> {
                let value = match T::take_from_reflect(value) {
                    Ok(v) => v,
                    Err(e) => return Err(e),
                };
                Ok(Self::insert(self, value))
            }

            fn remove(&mut self, value: &dyn $crate::Reflect) -> bool {
                let mut from_reflect = None;
                value
                    .downcast_ref::<T>()
                    .or_else(|| {
                        from_reflect = T::from_reflect(value);
                        from_reflect.as_ref()
                    })
                    .is_some_and(|value| self.remove(value))
            }

            fn contains(&self, value: &dyn $crate::Reflect) -> bool {
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

        impl<T> $crate::FromReflect for $ty
        where
            T: $crate::FromReflect + $crate::info::Typed + Eq + ::core::hash::Hash,
        {
            fn from_reflect(reflect: &dyn $crate::Reflect) -> Option<Self> {
                let ref_set = reflect.reflect_ref().as_set().ok()?;

                let mut new_set = Self::with_capacity(ref_set.len());

                for value in ref_set.iter() {
                    let new_value = T::from_reflect(value)?;
                    Self::insert(&mut new_set, new_value);
                }

                Some(new_set)
            }
        }

        impl<T> $crate::registry::GetTypeMeta for $ty
        where
            T: $crate::FromReflect
                + $crate::info::Typed
                + $crate::registry::GetTypeMeta
                + Eq
                + ::core::hash::Hash,
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
                registry.register::<T>();
            }
        }
    };
}

pub(crate) use impl_reflect_for_fixedhashset;
