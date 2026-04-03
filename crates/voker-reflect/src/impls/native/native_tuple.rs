//! Implement reflection traits for tuples with a field count of 12 or less.
//!
//! - [`TypePath`] -> [`DynamicTypePath`]
//! - [`Typed`] -> [`DynamicTyped`]
//! - [`Tuple`]
//! - [`Reflect`]
//! - [`GetTypeMeta`]
//! - [`FromReflect`]
//!
//! [`DynamicTypePath`]: crate::info::DynamicTypePath
//! [`DynamicTyped`]: crate::info::DynamicTyped

use alloc::{boxed::Box, vec, vec::Vec};
use core::cmp::Ordering;
use core::fmt;

use voker_utils::range_invoke;

use crate::impls::{GenericTypeInfoCell, GenericTypePathCell, NonGenericTypeInfoCell};
use crate::info::{TupleInfo, TypeInfo, TypePath, Typed, UnnamedField};
use crate::ops::{ApplyError, ReflectCloneError, Tuple, TupleFieldIter};
use crate::registry::{FromType, GetTypeMeta, TypeMeta, TypeRegistry};
use crate::registry::{ReflectDefault, ReflectFromPtr, ReflectFromReflect};
use crate::registry::{ReflectDeserialize, ReflectSerialize};
use crate::{FromReflect, Reflect};

macro_rules! impl_type_path_tuple {
    (0: []) => {
        impl TypePath for () {
            #[inline]
            fn type_path() -> &'static str {
                "()"
            }
            #[inline]
            fn type_name() -> &'static str {
                "()"
            }
            #[inline]
            fn type_ident() -> &'static str {
                "()"
            }
        }
    };
    (1: [ $index:tt : $name:ident ]) => {
        #[cfg_attr(docsrs, doc(fake_variadic))]
        impl<$name: TypePath> TypePath for ($name,) {
            fn type_path() -> &'static str {
                static CELL: GenericTypePathCell = GenericTypePathCell::new();
                CELL.get_or_insert::<Self>(|| {
                    $crate::impls::concat(&["(" , <$name>::type_path() , ",)"])
                })
            }

            fn type_name() -> &'static str {
                static CELL: GenericTypePathCell = GenericTypePathCell::new();
                CELL.get_or_insert::<Self>(|| {
                    $crate::impls::concat(&["(" , <$name>::type_name() , ",)"])
                })
            }

            fn type_ident() -> &'static str {
                static CELL: GenericTypePathCell = GenericTypePathCell::new();
                CELL.get_or_insert::<Self>(|| {
                    $crate::impls::concat(&["(" , <$name>::type_ident() , ",)"])
                })
            }
        }
    };
    ($_:literal: [$zero_index:tt : $zero_name:ident , $($index:tt : $name:ident),*]) => {
        #[cfg_attr(docsrs, doc(hidden))]
        impl<$zero_name: TypePath, $($name: TypePath),*> TypePath for ($zero_name, $($name),*) {
            fn type_path() -> &'static str {
                static CELL: GenericTypePathCell = GenericTypePathCell::new();
                CELL.get_or_insert::<Self>(|| {
                    $crate::impls::concat(&["(", <$zero_name>::type_path() $(, ", ", <$name>::type_path())* , ")"])
                })
            }

            fn type_name() -> &'static str {
                static CELL: GenericTypePathCell = GenericTypePathCell::new();
                CELL.get_or_insert::<Self>(|| {
                    $crate::impls::concat(&["(", <$zero_name>::type_name() $(, ", ", <$name>::type_name())* , ")"])
                })
            }

            fn type_ident() -> &'static str {
                static CELL: GenericTypePathCell = GenericTypePathCell::new();
                CELL.get_or_insert::<Self>(|| {
                    $crate::impls::concat(&["(", <$zero_name>::type_ident() $(, ", ", <$name>::type_ident())* , ")"])
                })
            }
        }
    };
}

range_invoke!(impl_type_path_tuple, 12);

macro_rules! impl_reflect_tuple {
    (0: []) => {
        impl Typed for () {
            fn type_info() -> &'static TypeInfo {
                static CELL: NonGenericTypeInfoCell = NonGenericTypeInfoCell::new();
                CELL.get_or_init(|| {
                    TypeInfo::Tuple(TupleInfo::new::<Self>(&[]))
                })
            }
        }

        impl Tuple for () {
            #[inline]
            fn field(&self, _index: usize) -> Option<&dyn Reflect> {
                None
            }

            #[inline]
            fn field_mut(&mut self, _index: usize) -> Option<&mut dyn Reflect> {
                None
            }

            #[inline]
            fn field_len(&self) -> usize {
                0
            }

            #[inline]
            fn iter_fields(&self) -> TupleFieldIter<'_> {
                TupleFieldIter::new(self)
            }

            #[inline]
            fn drain(self: Box<Self>) -> Vec<Box<dyn Reflect>> {
                Vec::new()
            }
        }

        impl Reflect for () {
            crate::reflection::impl_reflect_cast_fn!(Tuple);

            #[inline]
            fn apply(&mut self, value: &dyn Reflect) -> Result<(), ApplyError> {
                crate::impls::tuple_apply(self, value)
            }

            #[inline]
            fn reflect_eq(&self, other: &dyn Reflect) -> Option<bool> {
                if other.is::<Self>() {
                    Some(true)
                } else {
                    Some(false)
                }
            }

            #[inline]
            fn reflect_cmp(&self, other: &dyn Reflect) -> Option<Ordering> {
                if other.is::<Self>() {
                    Some(Ordering::Equal)
                } else {
                    None
                }
            }

            #[inline]
            fn to_dynamic(&self) -> Box<dyn Reflect> {
                Box::new(())
            }

            #[inline]
            fn reflect_clone(&self) -> Result<Box<dyn Reflect>, ReflectCloneError> {
                Ok(Box::new(()))
            }

            #[inline]
            fn reflect_hash(&self) -> Option<u64> {
                let mut hasher = crate::reflect_hasher();
                <() as core::hash::Hash>::hash(self, &mut hasher);
                Some(core::hash::Hasher::finish(&hasher))
            }

            #[inline]
            fn reflect_debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Debug::fmt(self, f)
            }
        }

        impl GetTypeMeta for () {
            fn get_type_meta() -> TypeMeta {
                let mut type_meta = TypeMeta::with_capacity::<Self>(5);
                type_meta.insert_data::<ReflectDefault>(FromType::<Self>::from_type());
                type_meta.insert_data::<ReflectFromPtr>(FromType::<Self>::from_type());
                type_meta.insert_data::<ReflectFromReflect>(FromType::<Self>::from_type());
                type_meta.insert_data::<ReflectSerialize>(FromType::<Self>::from_type());
                type_meta.insert_data::<ReflectDeserialize>(FromType::<Self>::from_type());
                type_meta
            }
        }

        impl FromReflect for () {
            fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
                let y = reflect.reflect_ref().as_tuple().ok()?;

                if 0 != y.field_len() {
                    return None;
                }

                Some(())
            }
        }
    };
    (1 : [ $index:tt : $name:ident ]) => {
        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 12 items long.")]
        impl<$name: Reflect + Typed> Typed for ($name,) {
            fn type_info() -> &'static TypeInfo {
                static CELL: GenericTypeInfoCell = GenericTypeInfoCell::new();
                CELL.get_or_insert::<Self>(|| {
                    TypeInfo::Tuple(TupleInfo::new::<Self>(&[
                        UnnamedField::new::<$name>($index)
                    ]))
                })
            }
        }

        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 12 items long.")]
        impl<$name: Reflect + Typed> Tuple for ($name,) {
            #[inline]
            fn field(&self, index: usize) -> Option<&dyn Reflect> {
                match index {
                    $index => Some(&self.$index as &dyn Reflect),
                    _ => None,
                }
            }

            #[inline]
            fn field_mut(&mut self, index: usize) -> Option<&mut dyn Reflect> {
                match index {
                    $index => Some(&mut self.$index as &mut dyn Reflect),
                    _ => None,
                }
            }

            #[inline]
            fn field_len(&self) -> usize {
                1
            }

            #[inline]
            fn iter_fields(&self) -> TupleFieldIter<'_> {
                TupleFieldIter::new(self)
            }

            #[inline]
            fn drain(self: Box<Self>) -> Vec<Box<dyn Reflect>> {
                vec![
                    Box::new(self.$index),
                ]
            }
        }

        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 12 items long.")]
        impl<$name: Reflect + Typed> Reflect for ($name,) {
            crate::reflection::impl_reflect_cast_fn!(Tuple);
            #[inline]
            fn apply(&mut self, value: &dyn Reflect) -> Result<(), ApplyError> {
                crate::impls::tuple_apply(self, value)
            }

            #[inline]
            fn reflect_eq(&self, other: &dyn Reflect) -> Option<bool> {
                crate::impls::tuple_eq(self, other)
            }

            #[inline]
            fn reflect_cmp(&self, other: &dyn Reflect) -> Option<Ordering> {
                crate::impls::tuple_cmp(self, other)
            }

            fn reflect_clone(&self) -> Result<Box<dyn Reflect>, ReflectCloneError> {
                Ok(Box::new((
                    self.$index.reflect_clone()?
                        .take::<$name>()
                        .expect("`Reflect::reflect_clone` should return the same type"),
                )))
            }

            #[inline]
            fn reflect_hash(&self) -> Option<u64> {
                crate::impls::tuple_hash(self)
            }

            #[inline]
            fn reflect_debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                crate::impls::tuple_debug(self, f)
            }
        }

        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 12 items long.")]
        impl<$name: Reflect + Typed + GetTypeMeta> GetTypeMeta for ($name,) {
            fn get_type_meta() -> TypeMeta {
                let mut type_meta =  TypeMeta::with_capacity::<($name,)>(1);
                type_meta.insert_data::<ReflectFromPtr>(FromType::<Self>::from_type());
                type_meta
            }

            fn register_dependencies(registry: &mut TypeRegistry) {
                registry.register::<$name>();
            }
        }

        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 12 items long.")]
        impl<$name: FromReflect + Typed> FromReflect for ($name,) {
            fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
                let _ref_tuple = reflect.reflect_ref().as_tuple().ok()?;

                if _ref_tuple.field_len() != 1 {
                    return None;
                }

                Some((<$name as FromReflect>::from_reflect(_ref_tuple.field($index)?)?,))
            }
        }
    };
    ($num:literal : [$($index:tt : $name:ident),*]) => {
        #[cfg_attr(docsrs, doc(hidden))]
        impl<$($name: Reflect + Typed),*> Typed for ($($name,)*) {
            fn type_info() -> &'static TypeInfo {
                static CELL: GenericTypeInfoCell = GenericTypeInfoCell::new();
                CELL.get_or_insert::<Self>(|| {
                    let fields = [
                        $(UnnamedField::new::<$name>($index),)*
                    ];
                    let info = TupleInfo::new::<Self>(&fields);
                    TypeInfo::Tuple(info)
                })
            }
        }

        #[cfg_attr(docsrs, doc(hidden))]
        impl<$($name: Reflect + Typed),*> Tuple for ($($name,)*) {
            #[inline]
            fn field(&self, index: usize) -> Option<&dyn Reflect> {
                match index {
                    $($index => Some(&self.$index as &dyn Reflect),)*
                    _ => None,
                }
            }

            #[inline]
            fn field_mut(&mut self, index: usize) -> Option<&mut dyn Reflect> {
                match index {
                    $($index => Some(&mut self.$index as &mut dyn Reflect),)*
                    _ => None,
                }
            }

            #[inline]
            fn field_len(&self) -> usize {
                $num
            }

            #[inline]
            fn iter_fields(&self) -> TupleFieldIter<'_> {
                TupleFieldIter::new(self)
            }

            #[inline]
            fn drain(self: Box<Self>) -> Vec<Box<dyn Reflect>> {
                vec![
                    $(Box::new(self.$index),)*
                ]
            }
        }

        #[cfg_attr(docsrs, doc(hidden))]
        impl<$($name: Reflect + Typed),*> Reflect for ($($name,)*) {
            crate::reflection::impl_reflect_cast_fn!(Tuple);

            #[inline]
            fn apply(&mut self, value: &dyn Reflect) -> Result<(), ApplyError> {
                crate::impls::tuple_apply(self, value)
            }

            #[inline]
            fn reflect_eq(&self, other: &dyn Reflect) -> Option<bool> {
                crate::impls::tuple_eq(self, other)
            }

            #[inline]
            fn reflect_cmp(&self, other: &dyn Reflect) -> Option<Ordering> {
                crate::impls::tuple_cmp(self, other)
            }

            fn reflect_clone(&self) -> Result<Box<dyn Reflect>, ReflectCloneError> {
                Ok(Box::new((
                    $(
                        self.$index.reflect_clone()?
                            .take::<$name>()
                            .expect("`Reflect::reflect_clone` should return the same type"),
                    )*
                )))
            }

            #[inline]
            fn reflect_hash(&self) -> Option<u64> {
                crate::impls::tuple_hash(self)
            }

            #[inline]
            fn reflect_debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                crate::impls::tuple_debug(self, f)
            }
        }

        #[cfg_attr(docsrs, doc(hidden))]
        impl<$($name: Reflect + Typed + GetTypeMeta),*> GetTypeMeta for ($($name,)*) {
            fn get_type_meta() -> TypeMeta {
                let mut type_meta =  TypeMeta::with_capacity::<($($name,)*)>(1);
                type_meta.insert_data::<ReflectFromPtr>(FromType::<Self>::from_type());
                type_meta
            }

            fn register_dependencies(_registry: &mut TypeRegistry) {
                $(_registry.register::<$name>();)*
            }
        }

        #[cfg_attr(docsrs, doc(hidden))]
        impl<$($name: FromReflect + Typed),*> FromReflect for ($($name,)*) {
            fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
                let _ref_tuple = reflect.reflect_ref().as_tuple().ok()?;

                if _ref_tuple.field_len() != $num {
                    return None;
                }

                Some((
                    $(
                        <$name as FromReflect>::from_reflect(_ref_tuple.field($index)?)?,
                    )*
                ))
            }
        }
    };
}

range_invoke!(impl_reflect_tuple, 12);

crate::derive::impl_auto_register!(());
