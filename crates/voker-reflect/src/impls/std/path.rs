use alloc::borrow::Cow;
use std::path::{Path, PathBuf};

use crate::{FromReflect, Reflect};
use crate::derive::{impl_reflect_opaque, impl_type_path};
use crate::info::{OpaqueInfo, TypeInfo, Typed};
use crate::registry::{FromType, GetTypeMeta, ReflectConvert, ReflectDefault, ReflectDeserialize};
use crate::registry::{ReflectFromReflect, ReflectSerialize, TypeMeta};

impl_reflect_opaque!{
    ::std::path::PathBuf(
        Default,
        Debug,
        Clone,
        Hash,
        PartialEq,
        PartialOrd,
        Serialize,
        Deserialize,
    )
}

impl_type_path!(::std::path::Path);

impl Typed for &'static Path {
    fn type_info() -> &'static TypeInfo {
        static INFO: TypeInfo = TypeInfo::Opaque(OpaqueInfo::new::<&'static Path>());
        &INFO
    }
}

impl Reflect for &'static Path {
    crate::impls::impl_simple_type_reflect!(Opaque);
}

impl FromReflect for &'static Path {
    #[inline]
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        reflect.downcast_ref::<Self>().copied()
    }
}

impl GetTypeMeta for &'static Path {
    fn get_type_meta() -> TypeMeta {
        let mut type_meta = TypeMeta::with_capacity::<Self>(3);
        type_meta.insert_data::<ReflectFromReflect>(FromType::<Self>::from_type());
        type_meta.insert_data::<ReflectSerialize>(FromType::<Self>::from_type());
        let mut converter = ReflectConvert::new::<Self>();
        converter.register_into::<Self, PathBuf>();
        converter.register_into::<Self, Cow<'static, Path>>();
        type_meta.insert_data(converter);
        type_meta
    }
}

impl Typed for Cow<'static, Path> {
    fn type_info() -> &'static TypeInfo {
        static INFO: TypeInfo = TypeInfo::Opaque(OpaqueInfo::new::<Cow<'static, Path>>());
        &INFO
    }
}

impl Reflect for Cow<'static, Path> {
    crate::impls::impl_simple_type_reflect!(Opaque);
}

impl FromReflect for Cow<'static, Path> {
    #[inline]
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        reflect.downcast_ref::<Self>().cloned()
    }
}

impl GetTypeMeta for Cow<'static, Path> {
    fn get_type_meta() -> TypeMeta {
        let mut type_meta: TypeMeta = TypeMeta::with_capacity::<Self>(5);
        type_meta.insert_data::<ReflectDefault>(FromType::<Self>::from_type());
        type_meta.insert_data::<ReflectFromReflect>(FromType::<Self>::from_type());
        type_meta.insert_data::<ReflectDeserialize>(FromType::<Self>::from_type());
        type_meta.insert_data::<ReflectSerialize>(FromType::<Self>::from_type());
        let mut converter = ReflectConvert::new::<Self>();
        converter.register_into::<Self, PathBuf>();
        converter.register_from::<Self, PathBuf>();
        converter.register_from::<Self, &'static Path>();
        type_meta.insert_data(converter);
        type_meta
    }
}

crate::derive::impl_auto_register!(Cow<'static, Path>);
crate::derive::impl_auto_register!(&'static Path);
