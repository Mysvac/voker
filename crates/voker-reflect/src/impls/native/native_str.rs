use crate::info::{OpaqueInfo, TypeInfo, TypePath, Typed};
use crate::registry::{FromType, GetTypeMeta, ReflectDefault, TypeMeta};
use crate::registry::{ReflectFromPtr, ReflectFromReflect, ReflectSerialize};
use crate::{FromReflect, Reflect};

impl TypePath for str {
    #[inline]
    fn type_path() -> &'static str {
        "str"
    }
    #[inline]
    fn type_name() -> &'static str {
        "str"
    }
    #[inline]
    fn type_ident() -> &'static str {
        "str"
    }
}

// impl TypePath for &'static str
// See `native_ref.rs`

impl Typed for &'static str {
    fn type_info() -> &'static TypeInfo {
        static INFO: TypeInfo = TypeInfo::Opaque(OpaqueInfo::new::<&'static str>());
        &INFO
    }
}

impl Reflect for &'static str {
    crate::impls::impl_simple_type_reflect!(Opaque);
}

impl GetTypeMeta for &'static str {
    fn get_type_meta() -> TypeMeta {
        let mut type_meta = TypeMeta::with_capacity::<Self>(4);
        type_meta.insert_data::<ReflectDefault>(FromType::<Self>::from_type());
        type_meta.insert_data::<ReflectFromPtr>(FromType::<Self>::from_type());
        type_meta.insert_data::<ReflectFromReflect>(FromType::<Self>::from_type());
        type_meta.insert_data::<ReflectSerialize>(FromType::<Self>::from_type());
        type_meta
    }
}

impl FromReflect for &'static str {
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        reflect.downcast_ref::<Self>().copied()
    }
}

crate::derive::impl_auto_register!(&'static str);
