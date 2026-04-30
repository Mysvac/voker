use alloc::boxed::Box;

use crate::Reflect;
use crate::ops::ReflectRef;

/// A trait that enables types to be dynamically constructed from reflected data.
///
/// It's recommended to use the [derive macro] rather than manually implementing this trait.
///
/// `FromReflect` allows dynamic proxy types, like [`DynamicStruct`], to be used to generate
/// their concrete counterparts.
///
/// In some cases, this trait may even be required. Deriving [`Reflect`] on an enum requires
/// all its fields to implement `FromReflect`. Additionally, some complex types like `Vec<T>`
/// require that their element types implement this trait.
///
/// The reason for such requirements is that some operations require new data to be constructed,
/// such as swapping to a new variant or pushing data to a homogeneous list.
///
/// `FromReflect` is intentionally non-panicking: conversion failures are represented
/// as `None`, which allows callers to compose fallback strategies.
///
/// # Rules
///
/// 1. For unit type, if `TypeId` matched, return a new value.
/// 2. For all other types, if `TypeId` matched, try to clone/reflect_clone and return.
/// 3. If `Self` is Opaque type or [`ReflectKind`] mismatched, return `None`.
/// 4. Otherwise:
///     - Set: Try to construct all values through [`from_reflect`].
///     - Map: Try to construct all key-values through [`from_reflect`].
///     - List: try to construct all items through [`from_reflect`].
///     - Enum: Try to construct all fields through [`from_reflect`] with specific enum variant.
///     - Array: If lengths matched, try to construct all items through [`from_reflect`].
///     - Tuple: If field lengths matched, try to construct all fields through [`from_reflect`].
///     - TupleStruct: If field lengths matched:
///         1. If Self support default (`reflect(Default)` flag), create a default value call [`apply`].
///         2. Otherwise try to construct all fields through [`from_reflect`].
///     - Struct:
///         1. If Self support default (`reflect(Default)` flag), create a default value, call [`apply`].
///         2. Try to construct all fields through [`from_reflect`].
///
/// `Struct` is the most special and may allow successful conversion between types
/// with different numbers of fields(if `reflect(Default)` attribute is marked).
///
/// # Examples
///
/// ```
/// use voker_reflect::{FromReflect, ops::DynamicStruct, Reflect};
///
/// #[derive(Reflect)]
/// struct A {
///     field_a: i32,
///     field_b: bool,
/// }
///
/// let mut dynamic = DynamicStruct::new();
/// dynamic.extend("field_a", 10_i32);
/// dynamic.extend("field_b", true);
///
///
/// let a = A::from_reflect(&dynamic).unwrap();
///
/// assert_eq!(a.field_a, 10);
/// assert_eq!(a.field_b, true);
/// ```
///
/// [`apply`]: Reflect::apply
/// [`reflect_clone`]: Reflect::reflect_clone
/// [`from_reflect`]: FromReflect::from_reflect
/// [`take_from_reflect`]: FromReflect::take_from_reflect
/// [`ReflectKind`]: crate::info::ReflectKind
/// [derive macro]: crate::derive::Reflect
/// [`DynamicStruct`]: crate::ops::DynamicStruct
/// [crate-level documentation]: crate
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement `FromReflect`",
    note = "consider annotating `{Self}` with `#[derive(Reflect)]`"
)]
pub trait FromReflect: Reflect + Sized {
    /// Constructs a concrete instance of `Self` from a reflected value.
    ///
    /// Implementations should return `None` for incompatible shape/kind/type data,
    /// instead of panicking.
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self>;

    /// Attempts to downcast the given value to `Self`; if that fails, try to construct
    /// the value using [`FromReflect::from_reflect`].
    ///
    /// Fast path:
    /// - If the boxed value already contains `Self`, this method consumes and returns it.
    ///
    /// Fallback path:
    /// - Calls [`FromReflect::from_reflect`] using a borrow.
    /// - On failure, returns the original `Box<dyn Reflect>` unchanged.
    fn take_from_reflect(reflect: Box<dyn Reflect>) -> Result<Self, Box<dyn Reflect>> {
        if reflect.is::<Self>() {
            #[expect(unsafe_code, reason = "already checked")]
            Ok(unsafe { *reflect.downcast::<Self>().unwrap_unchecked() })
        } else {
            match Self::from_reflect(reflect.as_ref()) {
                Some(success) => Ok(success),
                None => Err(reflect),
            }
        }
    }
}

impl FromReflect for crate::ops::DynamicStruct {
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        if let ReflectRef::Struct(val) = reflect.reflect_ref() {
            Some(val.to_dynamic_struct())
        } else {
            None
        }
    }
}

impl FromReflect for crate::ops::DynamicTupleStruct {
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        if let ReflectRef::TupleStruct(val) = reflect.reflect_ref() {
            Some(val.to_dynamic_tuple_struct())
        } else {
            None
        }
    }
}

impl FromReflect for crate::ops::DynamicTuple {
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        if let ReflectRef::Tuple(val) = reflect.reflect_ref() {
            Some(val.to_dynamic_tuple())
        } else {
            None
        }
    }
}

impl FromReflect for crate::ops::DynamicArray {
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        if let ReflectRef::Array(val) = reflect.reflect_ref() {
            Some(val.to_dynamic_array())
        } else {
            None
        }
    }
}

impl FromReflect for crate::ops::DynamicList {
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        if let ReflectRef::List(val) = reflect.reflect_ref() {
            Some(val.to_dynamic_list())
        } else {
            None
        }
    }
}

impl FromReflect for crate::ops::DynamicMap {
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        if let ReflectRef::Map(val) = reflect.reflect_ref() {
            Some(val.to_dynamic_map())
        } else {
            None
        }
    }
}

impl FromReflect for crate::ops::DynamicSet {
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        if let ReflectRef::Set(val) = reflect.reflect_ref() {
            Some(val.to_dynamic_set())
        } else {
            None
        }
    }
}

impl FromReflect for crate::ops::DynamicEnum {
    fn from_reflect(reflect: &dyn Reflect) -> Option<Self> {
        if let ReflectRef::Enum(val) = reflect.reflect_ref() {
            Some(val.to_dynamic_enum())
        } else {
            None
        }
    }
}

// -----------------------------------------------------------------------------
// tests

#[cfg(test)]
mod tests {
    use super::FromReflect;
    use crate::Reflect;
    use crate::ops::DynamicStruct;
    use alloc::boxed::Box;

    #[derive(Reflect, PartialEq, Debug)]
    struct Pair {
        left: i32,
        right: bool,
    }

    #[test]
    fn take_from_reflect() {
        let value = Box::new(Pair {
            left: 3,
            right: true,
        }) as Box<dyn Reflect>;

        let pair = Pair::take_from_reflect(value).unwrap();
        assert_eq!(
            pair,
            Pair {
                left: 3,
                right: true
            }
        );
    }

    #[test]
    fn from_reflect() {
        let mut dynamic = DynamicStruct::new();
        dynamic.extend("left", 9_i32);
        dynamic.extend("right", false);

        let pair = Pair::from_reflect(&dynamic).unwrap();
        assert_eq!(
            pair,
            Pair {
                left: 9,
                right: false
            }
        );
    }
}
