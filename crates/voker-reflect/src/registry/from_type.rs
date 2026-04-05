use crate::info::Typed;

/// Trait used to generate [`TypeData`] for trait reflection.
///
/// This is used by the `#[derive(Reflect)]` macro to generate an implementation
/// of [`TypeData`] to pass to [`TypeMeta::insert_data`].
///
/// # Example
///
/// ```
/// # use voker_reflect::registry::{TypeMeta, ReflectDefault, FromType};
/// let mut meta = TypeMeta::new::<String>();
///
/// meta.insert_data::<ReflectDefault>(FromType::<String>::from_type());
/// ```
///
/// [`TypeData`]: crate::registry::TypeData
/// [`TypeMeta::insert_data`]: crate::registry::TypeMeta::insert_data
pub trait FromType<T: Typed> {
    fn from_type() -> Self;
}
