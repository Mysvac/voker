use alloc::boxed::Box;
use core::any::TypeId;
use core::fmt::Debug;
use core::marker::PhantomData;

use voker_utils::debug::DebugName;
use voker_utils::extra::TypeIdMap;

use crate::Reflect;
use crate::info::TypePath;

// -----------------------------------------------------------------------------
// ReflectConvert

/// Provides a mechanism for converting values of one type to another.
///
/// This [`TypeData`] is associated with the target type being converted *into* and *from*.
/// Each `ReflectConvert` instance stores bidirectional conversion functions for
/// converting between its associated type and other types.
///
/// # Type Association
///
/// `ReflectConvert` is type-specific - each instance is associated with a concrete type `T`.
/// The stored conversion functions all have `T` as either the source or destination.
///
/// # Conversion Directions
///
/// - Use [`ReflectConvert::try_convert_from`] to convert a reflected value **into** the
///   associated type `T`
/// - Use [`ReflectConvert::try_convert_into`] to convert the associated type `T` **into**
///   another target type
///
/// # Identity Conversion
///
/// Conversion from `T` to itself always succeeds without requiring any registered
/// conversion function. This is handled as a fast-path optimization.
///
/// [`TypeData`]: crate::registry::TypeData
///
/// # Examples
///
/// Converting values using a registered type registry:
///
/// ```rust
/// use voker_reflect::prelude::*;
///
/// let mut registry = TypeRegistry::new();
/// registry.auto_register();
///
/// // Register standard `From` trait implementation
/// registry.register_type_from::<i32, isize>();
///
/// // Register custom conversion closure
/// registry.register_type_conversion(|x: i32| Ok(x.to_string()));
///
/// let type_id = TypeId::of::<i32>();
/// let converter = registry.get_type_data::<ReflectConvert>(type_id).unwrap();
///
/// // Convert from isize to i32
/// let i32_val = converter.try_convert_from(Box::new(1234_isize)).unwrap();
///
/// // Convert i32 to String
/// let str_val = converter.try_convert_into(i32_val, TypeId::of::<String>()).unwrap();
///
/// let converted: String = str_val.take::<String>().unwrap();
/// assert_eq!(converted, "1234");
/// ```
#[derive(Clone, TypePath)]
#[type_path = "voker_reflect::registry::ReflectConvert"]
pub struct ReflectConvert {
    this: TypeId,
    name: DebugName,
    into: TypeIdMap<Converter>,
    from: TypeIdMap<Converter>,
}

// -----------------------------------------------------------------------------
// Impl

trait CustomConverter: Send + Sync + 'static {
    fn clone_converter(&self) -> Box<dyn CustomConverter>;
    fn call(&self, src: Box<dyn Reflect>) -> Result<Box<dyn Reflect>, Box<dyn Reflect>>;
}

enum Converter {
    Func(fn(Box<dyn Reflect>) -> Result<Box<dyn Reflect>, Box<dyn Reflect>>),
    Wrap(Box<dyn CustomConverter>),
}

impl Converter {
    fn call(&self, src: Box<dyn Reflect>) -> Result<Box<dyn Reflect>, Box<dyn Reflect>> {
        match self {
            Converter::Func(func) => (*func)(src),
            Converter::Wrap(wrap) => wrap.call(src),
        }
    }
}

impl Clone for Converter {
    fn clone(&self) -> Self {
        match self {
            Self::Func(func) => Self::Func(*func),
            Self::Wrap(wrap) => Self::Wrap(wrap.clone_converter()),
        }
    }
}

// -----------------------------------------------------------------------------
// Basic Impl

impl ReflectConvert {
    /// Creates a new conversion registry for type `T`.
    pub const fn new<T: Reflect + TypePath>() -> Self {
        Self {
            this: TypeId::of::<T>(),
            name: DebugName::type_name::<T>(),
            into: TypeIdMap::new(),
            from: TypeIdMap::new(),
        }
    }

    /// Attempts to convert a reflected value into the associated type `T`.
    ///
    /// This method tries to convert `from` (which may already be of type `T` or
    /// some other type) into `T` using the registered conversion functions.
    ///
    /// # Returns
    ///
    /// - `Ok(Box<dyn Reflect>)`: Successfully converted value of type `T`.
    /// - `Err(Box<dyn Reflect>)`: Conversion failed, returns the original unconverted value.
    ///
    /// # Behavior
    ///
    /// 1. If `from` is already of type `T`, returns it immediately (identity conversion)
    /// 2. Otherwise, looks up a conversion function from the source type to `T`
    /// 3. If found, attempts the conversion
    /// 4. If not found, returns the original value in the `Err` variant
    pub fn try_convert_from(
        &self,
        from: Box<dyn Reflect>,
    ) -> Result<Box<dyn Reflect>, Box<dyn Reflect>> {
        let ty = from.as_ref().type_id();
        if ty == self.this {
            return Ok(from); // from self
        }
        match self.from.get(ty) {
            Some(cv) => cv.call(from),
            None => Err(from),
        }
    }

    /// Attempts to convert a value of the associated type `T` into another target type.
    ///
    /// This method expects `from` to be of type `T` and attempts to convert it to the
    /// type identified by `into`.
    ///
    /// # Arguments
    ///
    /// - `from`: A boxed reflected value that should be of type `T`
    /// - `into`: The [`TypeId`] of the target conversion type
    ///
    /// # Returns
    ///
    /// - `Ok(Box<dyn Reflect>)`: Successfully converted value of the target type
    /// - `Err(Box<dyn Reflect>)`: Conversion failed, returns the original value
    ///
    /// # Behavior
    ///
    /// 1. Validates that `from` is of type `T`; returns `Err` otherwise
    /// 2. If the target type is `T`, returns the value immediately (identity conversion)
    /// 3. Otherwise, looks up a conversion function from `T` to the target type
    /// 4. If found, attempts the conversion
    /// 5. If not found, returns the original value in the `Err` variant
    pub fn try_convert_into(
        &self,
        from: Box<dyn Reflect>,
        into: TypeId,
    ) -> Result<Box<dyn Reflect>, Box<dyn Reflect>> {
        let ty = from.as_ref().type_id();
        if ty != self.this {
            // `from` should match self type,
            return Err(from);
        }
        if into == self.this {
            return Ok(from); // into self
        }
        match self.into.get(into) {
            Some(cv) => cv.call(from),
            None => Err(from),
        }
    }
}

// -----------------------------------------------------------------------------
// Into & From Impl

#[cold]
#[inline(never)]
fn self_conversion(name: &str, info: &str) {
    tracing::warn!(
        "Try to register a self conversion `{info}` for type `{name}`, it's unapproved and skipped."
    )
}

impl ReflectConvert {
    /// Registers a conversion from `X` to `Y` using the [`Into`] trait.
    ///
    /// After calling this method, values of type `X` (the associated type of this
    /// `ReflectConvert` instance) can be converted into `Y`.
    ///
    /// Self conversion does not allow customization, skip it directly.
    ///
    /// # Type Parameters
    ///
    /// - `X`: The source type. Must equal the associated type `T` of this instance.
    /// - `Y`: The target type to convert into
    ///
    /// # Panics
    ///
    /// Panics if `X` does not match the associated type `T` of this `ReflectConvert` instance.
    ///
    /// Note: the priority of `Panic` > `Skip` (self conversion)
    pub fn register_into<X, Y>(&mut self)
    where
        X: Reflect + TypePath + Into<Y>,
        Y: Reflect + TypePath,
    {
        if self.this != TypeId::of::<X>() {
            register_into_failed(self.name, X::type_path(), Y::type_path());
        }

        if TypeId::of::<X>() == TypeId::of::<Y>() {
            self_conversion(X::type_path(), "Into::into");
            return;
        }

        let cv = Converter::Func(reflect_into::<X, Y>);
        self.into.insert(TypeId::of::<Y>(), cv);
    }

    /// Registers a conversion from `Y` to `X` using the [`From`] trait.
    ///
    /// This method registers the reverse direction of [`register_into`](Self::register_into).
    /// After calling, values of type `Y` can be converted into `X` (the associated type).
    ///
    /// Self conversion does not allow customization, skip it directly.
    ///
    /// # Type Parameters
    ///
    /// - `X`: The target type (must equal the associated type `T` of this instance)
    /// - `Y`: The source type to convert from
    ///
    /// # Panics
    ///
    /// Panics if `X` does not match the associated type `T` of this `ReflectConvert` instance.
    ///
    /// Note: the priority of `Panic` > `Skip` (self conversion)
    pub fn register_from<X, Y>(&mut self)
    where
        X: Reflect + TypePath + From<Y>,
        Y: Reflect + TypePath,
    {
        if self.this != TypeId::of::<X>() {
            register_from_failed(self.name, X::type_path(), Y::type_path());
        }

        if TypeId::of::<X>() == TypeId::of::<Y>() {
            self_conversion(X::type_path(), "From::from");
            return;
        }

        let cv = Converter::Func(reflect_into::<Y, X>); // `X from Y` == `Y into X`
        self.from.insert(TypeId::of::<Y>(), cv);
    }
}

fn reflect_into<X, Y>(src: Box<dyn Reflect>) -> Result<Box<dyn Reflect>, Box<dyn Reflect>>
where
    X: Reflect + TypePath + Into<Y>,
    Y: Reflect + TypePath,
{
    let x: Box<X> = src.downcast::<X>()?;
    let y: Box<Y> = Box::new((*x).into());
    Ok(y)
}

#[cold]
#[inline(never)]
fn register_into_failed(name: DebugName, src: &str, dst: &str) -> ! {
    panic!(
        "register_into failed, ReflectConvert type is `{name}`, Converter is `<{src} as Into<{dst}>>::into`"
    )
}

#[cold]
#[inline(never)]
fn register_from_failed(name: DebugName, src: &str, dst: &str) -> ! {
    panic!(
        "register_from failed, ReflectConvert type is `{name}`, Converter is `<{src} as From<{dst}>>::from`"
    )
}

// -----------------------------------------------------------------------------
// Impl

struct TypedConverter<T, U, F>
where
    T: Reflect + TypePath,
    U: Reflect + TypePath,
    F: Fn(T) -> Result<U, T> + Clone + Send + Sync + 'static,
{
    function: F,
    phantom: PhantomData<(T, U)>,
}

impl<T, U, F> CustomConverter for TypedConverter<T, U, F>
where
    T: Reflect + TypePath,
    U: Reflect + TypePath,
    F: Fn(T) -> Result<U, T> + Clone + Send + Sync + 'static,
{
    fn call(&self, input: Box<dyn Reflect>) -> Result<Box<dyn Reflect>, Box<dyn Reflect>> {
        let input: Box<T> = input.downcast::<T>()?;
        match (self.function)(*input) {
            Ok(value) => Ok(Box::new(value)),
            Err(value) => Err(Box::new(value)),
        }
    }

    fn clone_converter(&self) -> Box<dyn CustomConverter> {
        Box::new(Self {
            function: self.function.clone(),
            phantom: PhantomData,
        })
    }
}

impl ReflectConvert {
    /// Registers a custom fallible conversion from `X` to `Y`.
    ///
    /// Unlike [`register_into`](Self::register_into) which uses the [`Into`] trait,
    /// this method accepts an arbitrary closure that can fail and return the original
    /// value on error.
    ///
    /// Self conversion does not allow customization, skip it directly.
    ///
    /// # Type Parameters
    ///
    /// - `X`: The source type (must equal the associated type `T` of this instance)
    /// - `Y`: The target type
    /// - `F`: The conversion function type (must be `Fn(X) -> Result<Y, X>`)
    ///
    /// # Requirements
    ///
    /// - `F` must be `Clone + Send + Sync + 'static`
    /// - The conversion function returns `Result<Y, X>`, where:
    ///   - `Ok(Y)` represents a successful conversion
    ///   - `Err(X)` represents a failed conversion, returning the original value
    ///
    /// # Panics
    ///
    /// Panics if `X` does not match the associated type `T`.
    ///
    /// Note: the priority of `Panic` > `Skip` (self conversion)
    pub fn register_custom_into<X, Y, F>(&mut self, f: F)
    where
        X: Reflect + TypePath,
        Y: Reflect + TypePath,
        F: Fn(X) -> Result<Y, X> + Clone + Send + Sync + 'static,
    {
        if self.this != TypeId::of::<X>() {
            register_custom_failed(self.name, core::any::type_name::<F>());
        }

        if TypeId::of::<X>() == TypeId::of::<Y>() {
            self_conversion(X::type_path(), core::any::type_name::<F>());
            return;
        }

        let cv = TypedConverter::<X, Y, F> {
            function: f,
            phantom: PhantomData,
        };

        self.into.insert(TypeId::of::<Y>(), Converter::Wrap(Box::new(cv)));
    }

    /// Registers a custom fallible conversion from `Y` to `X`.
    ///
    /// This is the reverse direction of [`register_custom_into`](Self::register_custom_into).
    /// Converts values of type `Y` into the associated type `X`.
    ///
    /// Self conversion does not allow customization, skip it directly.
    ///
    /// # Type Parameters
    ///
    /// - `X`: The target type (must equal the associated type `T` of this instance)
    /// - `Y`: The source type
    /// - `F`: The conversion function type (must be `Fn(Y) -> Result<X, Y>`)
    ///
    /// # Requirements
    ///
    /// - `F` must be `Clone + Send + Sync + 'static`
    ///
    /// # Panics
    ///
    /// Panics if `X` does not match the associated type `T`.
    ///
    /// Note: the priority of `Panic` > `Skip` (self conversion)
    pub fn register_custom_from<X, Y, F>(&mut self, f: F)
    where
        X: Reflect + TypePath,
        Y: Reflect + TypePath,
        F: Fn(Y) -> Result<X, Y> + Clone + Send + Sync + 'static,
    {
        if self.this != TypeId::of::<X>() {
            register_custom_failed(self.name, core::any::type_name::<F>());
        }

        if TypeId::of::<X>() == TypeId::of::<Y>() {
            self_conversion(X::type_path(), core::any::type_name::<F>());
            return;
        }

        let cv = TypedConverter::<Y, X, F> {
            function: f,
            phantom: PhantomData,
        };

        self.from.insert(TypeId::of::<Y>(), Converter::Wrap(Box::new(cv)));
    }
}

#[cold]
#[inline(never)]
fn register_custom_failed(name: DebugName, src: &str) -> ! {
    panic!("register_custom_convert failed, ReflectConvert type is `{name}`, Converter is `{src}`")
}

// -----------------------------------------------------------------------------
// Traits

impl Debug for ReflectConvert {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ReflectConvert")
            .field("type", &self.name)
            .field("into", &self.into.len())
            .field("from", &self.from.len())
            .finish()
    }
}
