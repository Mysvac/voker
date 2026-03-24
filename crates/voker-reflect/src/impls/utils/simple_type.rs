macro_rules! impl_simple_type_reflect {
    ($kind:ident) => {
        $crate::reflection::impl_reflect_cast_fn!($kind);

        #[inline]
        fn to_dynamic(&self) -> ::alloc::boxed::Box<dyn $crate::Reflect> {
            ::alloc::boxed::Box::new(Clone::clone(self))
        }

        #[inline]
        fn reflect_clone(
            &self,
        ) -> Result<::alloc::boxed::Box<dyn $crate::Reflect>, $crate::ops::ReflectCloneError> {
            Ok(::alloc::boxed::Box::new(Clone::clone(self)))
        }

        fn apply(&mut self, value: &dyn $crate::Reflect) -> Result<(), $crate::ops::ApplyError> {
            if let Some(value) = <dyn $crate::Reflect>::downcast_ref::<Self>(value) {
                Clone::clone_from(self, value);
                Ok(())
            } else {
                Err($crate::ops::ApplyError::MismatchedType {
                    from_type: ::alloc::borrow::Cow::Borrowed(
                        $crate::info::DynamicTypePath::reflect_type_path(value),
                    ),
                    to_type: ::alloc::borrow::Cow::Borrowed(
                        <Self as $crate::info::TypePath>::type_path(),
                    ),
                })
            }
        }

        fn reflect_eq(&self, value: &dyn $crate::Reflect) -> Option<bool> {
            if let Some(value) = <dyn $crate::Reflect>::downcast_ref::<Self>(value) {
                Some(PartialEq::eq(self, value))
            } else {
                Some(false)
            }
        }

        fn reflect_cmp(&self, value: &dyn $crate::Reflect) -> Option<::core::cmp::Ordering> {
            if let Some(value) = <dyn $crate::Reflect>::downcast_ref::<Self>(value) {
                Some(Ord::cmp(self, value))
            } else {
                None
            }
        }

        fn reflect_hash(&self) -> Option<u64> {
            let mut hasher = $crate::reflect_hasher();
            <Self as ::core::hash::Hash>::hash(self, &mut hasher);
            Some(::core::hash::Hasher::finish(&hasher))
        }

        fn reflect_debug(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            ::core::fmt::Debug::fmt(self, f)
        }
    };
}

pub(crate) use impl_simple_type_reflect;
