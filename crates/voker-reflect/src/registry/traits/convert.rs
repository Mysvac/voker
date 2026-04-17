use alloc::boxed::Box;
use core::any::TypeId;
use core::marker::PhantomData;

use voker_utils::extra::TypeIdMap;

use crate::{Reflect, info::TypePath};

/// Type data that stores runtime conversion routes into a target reflected type.
///
/// This type is registered on the target type (`U`), not the source type (`T`).
/// Conversion attempts are dispatched by source `TypeId`.
#[derive(Default)]
pub struct ReflectConvert {
    conversions: TypeIdMap<Box<dyn Converter>>,
}

trait Converter: Send + Sync {
    fn convert(&self, input: Box<dyn Reflect>) -> Result<Box<dyn Reflect>, Box<dyn Reflect>>;
    fn clone_converter(&self) -> Box<dyn Converter>;
}

struct TypedConverter<T, U, F>
where
    T: Reflect + TypePath,
    U: Reflect + TypePath,
    F: Fn(T) -> Result<U, T> + Clone + Send + Sync + 'static,
{
    function: F,
    marker: PhantomData<(T, U)>,
}

impl ReflectConvert {
    /// Attempts to convert `input` into the target type associated with this type data.
    ///
    /// Returns the converted value on success, otherwise returns the original `input`.
    pub fn try_convert_from(
        &self,
        input: Box<dyn Reflect>,
    ) -> Result<Box<dyn Reflect>, Box<dyn Reflect>> {
        let ty = input.as_ref().type_id();

        match self.conversions.get(ty) {
            Some(converter) => converter.convert(input),
            None => Err(input),
        }
    }

    /// Registers a fallible conversion from `T` into `U`.
    ///
    /// The function should return `Ok(U)` for successful conversion,
    /// and `Err(T)` to return ownership of the original input on failure.
    pub fn register_type_conversion<T, U, F>(&mut self, function: F)
    where
        T: Reflect + TypePath,
        U: Reflect + TypePath,
        F: Fn(T) -> Result<U, T> + Clone + Send + Sync + 'static,
    {
        self.conversions.insert(
            TypeId::of::<T>(),
            Box::new(TypedConverter::<T, U, F> {
                function,
                marker: PhantomData,
            }),
        );
    }
}

impl Clone for ReflectConvert {
    fn clone(&self) -> Self {
        let mut conversions = TypeIdMap::with_capacity(self.conversions.len());
        for (id, converter) in self.conversions.iter() {
            conversions.insert(id, converter.clone_converter());
        }

        Self { conversions }
    }
}

impl<T, U, F> Clone for TypedConverter<T, U, F>
where
    T: Reflect + TypePath,
    U: Reflect + TypePath,
    F: Fn(T) -> Result<U, T> + Clone + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            function: self.function.clone(),
            marker: PhantomData,
        }
    }
}

impl<T, U, F> Converter for TypedConverter<T, U, F>
where
    T: Reflect + TypePath,
    U: Reflect + TypePath,
    F: Fn(T) -> Result<U, T> + Clone + Send + Sync + 'static,
{
    fn convert(&self, input: Box<dyn Reflect>) -> Result<Box<dyn Reflect>, Box<dyn Reflect>> {
        let mut input = input.downcast::<T>()?;

        match (self.function)(*input) {
            Ok(value) => Ok(Box::new(value)),
            Err(value) => {
                *input = value;
                Err(input)
            }
        }
    }

    fn clone_converter(&self) -> Box<dyn Converter> {
        Box::new(self.clone())
    }
}

impl TypePath for ReflectConvert {
    fn type_path() -> &'static str {
        "voker_reflect::registry::ReflectConvert"
    }

    fn type_name() -> &'static str {
        "ReflectConvert"
    }

    fn type_ident() -> &'static str {
        "ReflectConvert"
    }

    fn module_path() -> Option<&'static str> {
        Some("voker_reflect::registry")
    }
}
