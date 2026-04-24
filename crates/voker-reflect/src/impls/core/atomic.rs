//! Even if the atomic variable uses `portable_atomic`, its type path is still `core::sync::atomic::...`.

use alloc::boxed::Box;
use core::sync::atomic::Ordering;

use crate::derive::{impl_auto_register, impl_reflect, impl_type_path};

impl_reflect! {
    #[type_path = "core::sync::atomic::Ordering"]
    #[reflect(Clone, Debug, Hash, PartialEq)]
    pub enum Ordering {
        Relaxed,
        Release,
        Acquire,
        AcqRel,
        SeqCst,
    }
}

macro_rules! impl_reflect_for_atomic {
    ($ty:ty, $ordering:expr) => {
        // impl_type_path!($ty);

        impl_auto_register!($ty);

        impl $crate::info::Typed for $ty {
            fn type_info() -> &'static $crate::info::TypeInfo {
                static INFO: $crate::info::TypeInfo =
                    $crate::info::TypeInfo::Opaque($crate::info::OpaqueInfo::new::<$ty>());
                &INFO
            }
        }

        impl $crate::Reflect for $ty {
            crate::reflection::impl_reflect_cast_fn!(Opaque);

            #[inline]
            fn to_dynamic(&self) -> Box<dyn $crate::Reflect> {
                Box::new(<$ty>::new(self.load($ordering)))
            }

            #[inline]
            fn reflect_clone(
                &self,
            ) -> Result<Box<dyn $crate::Reflect>, $crate::ops::ReflectCloneError> {
                Ok(Box::new(<$ty>::new(self.load($ordering))))
            }

            fn apply(
                &mut self,
                value: &dyn $crate::Reflect,
            ) -> Result<(), $crate::ops::ApplyError> {
                if let Some(value) = value.downcast_ref::<Self>() {
                    *self = <$ty>::new(value.load($ordering));
                    Ok(())
                } else {
                    Err($crate::ops::ApplyError::MismatchedType {
                        from_type: Into::into($crate::info::DynamicTypePath::reflect_type_path(
                            value,
                        )),
                        to_type: Into::into(<Self as $crate::info::TypePath>::type_path()),
                    })
                }
            }

            fn reflect_debug(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                core::fmt::Debug::fmt(self, f)
            }
        }

        impl $crate::FromReflect for $ty {
            fn from_reflect(reflect: &dyn $crate::Reflect) -> Option<Self> {
                Some(<$ty>::new(reflect.downcast_ref::<$ty>()?.load($ordering)))
            }
        }

        impl $crate::registry::GetTypeMeta for $ty {
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
        }
    };
}

impl_type_path!((in core::sync::atomic as AtomicI8) voker_os::atomic::AtomicI8);
impl_type_path!((in core::sync::atomic as AtomicU8) voker_os::atomic::AtomicU8);
impl_type_path!((in core::sync::atomic as AtomicI16) voker_os::atomic::AtomicI16);
impl_type_path!((in core::sync::atomic as AtomicU16) voker_os::atomic::AtomicU16);
impl_type_path!((in core::sync::atomic as AtomicI32) voker_os::atomic::AtomicI32);
impl_type_path!((in core::sync::atomic as AtomicU32) voker_os::atomic::AtomicU32);
impl_type_path!((in core::sync::atomic as AtomicI64) voker_os::atomic::AtomicI64);
impl_type_path!((in core::sync::atomic as AtomicU64) voker_os::atomic::AtomicU64);
impl_type_path!((in core::sync::atomic as AtomicIsize) voker_os::atomic::AtomicIsize);
impl_type_path!((in core::sync::atomic as AtomicUsize) voker_os::atomic::AtomicUsize);
impl_type_path!((in core::sync::atomic as AtomicBool) voker_os::atomic::AtomicBool);
// impl_type_path!((in core::sync::atomic as AtomicPtr) voker_os::atomic::AtomicPtr<T>);

impl_reflect_for_atomic!(::voker_os::atomic::AtomicBool, Ordering::SeqCst);
impl_reflect_for_atomic!(::voker_os::atomic::AtomicI8, Ordering::SeqCst);
impl_reflect_for_atomic!(::voker_os::atomic::AtomicU8, Ordering::SeqCst);
impl_reflect_for_atomic!(::voker_os::atomic::AtomicI16, Ordering::SeqCst);
impl_reflect_for_atomic!(::voker_os::atomic::AtomicU16, Ordering::SeqCst);
impl_reflect_for_atomic!(::voker_os::atomic::AtomicI32, Ordering::SeqCst);
impl_reflect_for_atomic!(::voker_os::atomic::AtomicU32, Ordering::SeqCst);
impl_reflect_for_atomic!(::voker_os::atomic::AtomicI64, Ordering::SeqCst);
impl_reflect_for_atomic!(::voker_os::atomic::AtomicU64, Ordering::SeqCst);
impl_reflect_for_atomic!(::voker_os::atomic::AtomicIsize, Ordering::SeqCst);
impl_reflect_for_atomic!(::voker_os::atomic::AtomicUsize, Ordering::SeqCst);
