use alloc::borrow::{Cow, ToOwned};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cmp::Ordering;

use crate::derive::impl_type_path;
use crate::impls::GenericTypeInfoCell;
use crate::info::{ListInfo, OpaqueInfo, TypeInfo, TypePath, Typed};
use crate::ops::{ApplyError, List, ListItemIter};
use crate::registry::{
    FromType, GetTypeMeta, ReflectDefault, ReflectFromPtr, TypeMeta, TypeRegistry,
};
use crate::registry::{ReflectDeserialize, ReflectFromReflect, ReflectSerialize};
use crate::{FromReflect, Reflect};

// -----------------------------------------------------------------------------
// Cow<'static, str>

impl_type_path!(::alloc::borrow::Cow<'a: 'static, T: ToOwned + ?Sized>);

impl Typed for Cow<'static, str> {
    fn type_info() -> &'static TypeInfo {
        static INFO: TypeInfo = TypeInfo::Opaque(OpaqueInfo::new::<Cow<'static, str>>());
        &INFO
    }
}

impl Reflect for Cow<'static, str> {
    crate::reflection::impl_reflect_cast_fn!(Opaque);

    fn to_dynamic(&self) -> Box<dyn Reflect> {
        Box::new(self.clone())
    }

    fn apply(&mut self, value: &dyn Reflect) -> Result<(), ApplyError> {
        if let Some(value) = value.downcast_ref::<Self>() {
            self.clone_from(value);
        } else {
            return Err(ApplyError::MismatchedType {
                from_type: value.reflect_type_path().into(),
                to_type: <Self as TypePath>::type_path().into(),
            });
        }
        Ok(())
    }

    fn reflect_clone(&self) -> Result<Box<dyn Reflect>, crate::ops::ReflectCloneError> {
        Ok(Box::new(self.clone()))
    }

    fn reflect_eq(&self, other: &dyn Reflect) -> Option<bool> {
        if let Some(other) = other.downcast_ref::<Self>() {
            Some(PartialEq::eq(self, other))
        } else {
            Some(false)
        }
    }

    fn reflect_cmp(&self, other: &dyn Reflect) -> Option<Ordering> {
        other.downcast_ref::<Self>().map(|other| Ord::cmp(self, other))
    }

    fn reflect_hash(&self) -> Option<u64> {
        let mut hasher = crate::reflect_hasher();
        core::hash::Hash::hash(self, &mut hasher);
        Some(core::hash::Hasher::finish(&hasher))
    }

    fn reflect_debug(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self, f)
    }
}

impl GetTypeMeta for Cow<'static, str> {
    fn get_type_meta() -> TypeMeta {
        let mut meta = TypeMeta::with_capacity::<Self>(5);
        meta.insert_data::<ReflectFromPtr>(FromType::<Self>::from_type());
        meta.insert_data::<ReflectFromReflect>(FromType::<Self>::from_type());
        meta.insert_data::<ReflectDefault>(FromType::<Self>::from_type());
        meta.insert_data::<ReflectSerialize>(FromType::<Self>::from_type());
        meta.insert_data::<ReflectDeserialize>(FromType::<Self>::from_type());
        meta
    }
}

impl FromReflect for Cow<'static, str> {
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        Some(reflect.downcast_ref::<Self>()?.clone())
    }
}

crate::derive::impl_auto_register!(Cow<'static, str>);

// -----------------------------------------------------------------------------
// Cow<'static, [T]>

impl<T: FromReflect + Typed + Clone> Typed for Cow<'static, [T]> {
    fn type_info() -> &'static TypeInfo {
        static CELL: GenericTypeInfoCell = GenericTypeInfoCell::new();
        CELL.get_or_insert::<Self>(|| TypeInfo::List(ListInfo::new::<Self, T>()))
    }
}

impl<T: FromReflect + Typed + Clone> Reflect for Cow<'static, [T]> {
    crate::reflection::impl_reflect_cast_fn!(List);

    fn apply(&mut self, value: &dyn Reflect) -> Result<(), ApplyError> {
        crate::impls::list_apply(self, value)
    }

    fn to_dynamic(&self) -> Box<dyn Reflect> {
        Box::new(self.clone())
    }
    fn reflect_clone(&self) -> Result<Box<dyn Reflect>, crate::ops::ReflectCloneError> {
        Ok(Box::new(self.clone()))
    }

    fn reflect_eq(&self, value: &dyn Reflect) -> Option<bool> {
        crate::impls::list_eq(self, value)
    }

    fn reflect_cmp(&self, value: &dyn Reflect) -> Option<Ordering> {
        crate::impls::list_cmp(self, value)
    }

    fn reflect_hash(&self) -> Option<u64> {
        crate::impls::list_hash(self)
    }

    fn reflect_debug(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        crate::impls::list_debug(self, f)
    }
}

impl<T: FromReflect + Typed + Clone> List for Cow<'static, [T]> {
    fn get(&self, index: usize) -> Option<&dyn Reflect> {
        <[T]>::get(self.as_ref(), index).map(Reflect::as_reflect)
    }

    fn get_mut(&mut self, index: usize) -> Option<&mut dyn Reflect> {
        <[T]>::get_mut(self.to_mut(), index).map(Reflect::as_mut_reflect)
    }

    fn push(&mut self, value: Box<dyn Reflect>) {
        let value = match T::take_from_reflect(value) {
            Ok(v) => v,
            Err(e) => panic! {
                "incompatible type: from {} to {}",
                e.reflect_type_path(),
                T::type_path(),
            },
        };
        self.to_mut().push(value);
    }

    fn try_push(&mut self, value: Box<dyn Reflect>) -> Result<(), Box<dyn Reflect>> {
        let value = T::take_from_reflect(value)?;
        self.to_mut().push(value);
        Ok(())
    }

    fn pop(&mut self) -> Option<Box<dyn Reflect>> {
        self.to_mut().pop().map(Reflect::into_boxed_reflect)
    }

    fn len(&self) -> usize {
        self.as_ref().len()
    }

    fn is_empty(&self) -> bool {
        self.as_ref().is_empty()
    }

    fn iter(&self) -> ListItemIter<'_> {
        ListItemIter::new(self)
    }

    fn drain(&mut self) -> Vec<Box<dyn Reflect>> {
        self.to_mut().drain(..).map(Reflect::into_boxed_reflect).collect()
    }
}

impl<T: FromReflect + Typed + Clone> FromReflect for Cow<'static, [T]> {
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        let ref_list = reflect.reflect_ref().as_list().ok()?;

        let mut temp_vec = Vec::with_capacity(ref_list.len());

        for field in ref_list.iter() {
            temp_vec.push(T::from_reflect(field)?);
        }

        Some(temp_vec.into())
    }
}

impl<T: FromReflect + Typed + Clone + GetTypeMeta> GetTypeMeta for Cow<'static, [T]> {
    fn get_type_meta() -> TypeMeta {
        let mut meta = TypeMeta::with_capacity::<Self>(3);
        meta.insert_data::<ReflectDefault>(FromType::<Self>::from_type());
        meta.insert_data::<ReflectFromPtr>(FromType::<Self>::from_type());
        meta.insert_data::<ReflectFromReflect>(FromType::<Self>::from_type());
        meta
    }

    fn register_dependencies(registry: &mut TypeRegistry) {
        registry.register::<T>();
    }
}
