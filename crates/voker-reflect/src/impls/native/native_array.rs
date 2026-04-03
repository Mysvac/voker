use crate::{
    FromReflect, Reflect,
    impls::{GenericTypeInfoCell, GenericTypePathCell},
    info::{ArrayInfo, TypeInfo, TypePath, Typed},
    ops::{Array, ArrayItemIter, ReflectCloneError},
    registry::{FromType, GetTypeMeta, ReflectFromPtr, TypeMeta, TypeRegistry},
};
use alloc::{borrow::ToOwned, boxed::Box, string::ToString, vec::Vec};

impl<T: TypePath> TypePath for [T]
where
    [T]: ToOwned,
{
    fn type_path() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self>(|| crate::impls::concat(&["[", <T>::type_path(), "]"]))
    }

    fn type_name() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self>(|| crate::impls::concat(&["[", <T>::type_name(), "]"]))
    }

    fn type_ident() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self>(|| crate::impls::concat(&["[", <T>::type_ident(), "]"]))
    }
}

impl<T: TypePath, const N: usize> TypePath for [T; N] {
    fn type_path() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self>(|| {
            crate::impls::concat(&["[", T::type_path(), "; ", &N.to_string(), "]"])
        })
    }

    fn type_name() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self>(|| {
            crate::impls::concat(&["[", T::type_name(), "; ", &N.to_string(), "]"])
        })
    }

    fn type_ident() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self>(|| {
            crate::impls::concat(&["[", T::type_ident(), "; ", &N.to_string(), "]"])
        })
    }
}

impl<T: Reflect + Typed, const N: usize> Typed for [T; N] {
    fn type_info() -> &'static TypeInfo {
        static CELL: GenericTypeInfoCell = GenericTypeInfoCell::new();
        CELL.get_or_insert::<Self>(|| TypeInfo::Array(ArrayInfo::new::<Self, T>(N)))
    }
}

impl<T: Reflect + Typed, const N: usize> Reflect for [T; N] {
    crate::reflection::impl_reflect_cast_fn!(Array);

    #[inline]
    fn apply(&mut self, value: &dyn Reflect) -> Result<(), crate::ops::ApplyError> {
        crate::impls::array_apply(self, value)
    }

    #[inline]
    fn to_dynamic(&self) -> Box<dyn Reflect> {
        Box::new(Array::to_dynamic_array(self))
    }

    #[inline]
    fn reflect_clone(&self) -> Result<Box<dyn Reflect>, ReflectCloneError> {
        let res: Vec<T> = self
            .iter()
            .map(|item| {
                item.reflect_clone().map(|v| {
                    v.take::<T>()
                        .expect("`Reflect::reflect_clone` should return the same type")
                })
            })
            .collect::<Result<Vec<T>, ReflectCloneError>>()?;

        let res: Self = res.try_into().map_err(|_| ReflectCloneError::NotSupport {
            type_path: T::type_path(),
        })?;

        Ok(Box::new(res))
    }

    #[inline]
    fn reflect_hash(&self) -> Option<u64> {
        crate::impls::array_hash(self)
    }

    #[inline]
    fn reflect_eq(&self, value: &dyn Reflect) -> Option<bool> {
        crate::impls::array_eq(self, value)
    }

    #[inline]
    fn reflect_cmp(&self, value: &dyn Reflect) -> Option<core::cmp::Ordering> {
        crate::impls::array_cmp(self, value)
    }

    #[inline]
    fn reflect_debug(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        crate::impls::array_debug(self, f)
    }
}

impl<T: Reflect + Typed, const N: usize> Array for [T; N] {
    #[inline]
    fn get(&self, index: usize) -> Option<&dyn Reflect> {
        <[T]>::get(self, index).map(Reflect::as_reflect)
    }

    #[inline]
    fn get_mut(&mut self, index: usize) -> Option<&mut dyn Reflect> {
        <[T]>::get_mut(self, index).map(Reflect::as_mut_reflect)
    }

    #[inline]
    fn len(&self) -> usize {
        N
    }

    #[inline]
    fn iter(&self) -> ArrayItemIter<'_> {
        ArrayItemIter::new(self)
    }

    fn drain(self: Box<Self>) -> Vec<Box<dyn Reflect>> {
        self.into_iter().map(Reflect::into_boxed_reflect).collect()
    }
}

impl<T: Reflect + Typed + GetTypeMeta, const N: usize> GetTypeMeta for [T; N] {
    fn get_type_meta() -> TypeMeta {
        let mut type_meta = TypeMeta::with_capacity::<[T; N]>(1);
        type_meta.insert_data::<ReflectFromPtr>(FromType::<Self>::from_type());
        type_meta
    }

    fn register_dependencies(registry: &mut TypeRegistry) {
        registry.register::<T>();
    }
}

impl<T: FromReflect + Typed, const N: usize> FromReflect for [T; N] {
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        let ref_array = reflect.reflect_ref().as_array().ok()?;

        let len = ref_array.len();

        if len != N {
            return None;
        }

        let mut temp_vec: Vec<T> = Vec::with_capacity(len);

        for item in ref_array.iter() {
            temp_vec.push(T::from_reflect(item)?);
        }

        temp_vec.try_into().ok()
    }
}
