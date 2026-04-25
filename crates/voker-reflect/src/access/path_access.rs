//! Provides reusable and one-shot multi-layer path access support.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;

use voker_utils::vec::FastVec;

use crate::Reflect;
use crate::access::{AccessError, AccessPath, OffsetAccessor, ParseError};
use crate::ops::{Array, Enum, List, Struct, Tuple, TupleStruct};

// -----------------------------------------------------------------------------
// Error

/// An error returned from a failed path access.
#[derive(Debug, PartialEq, Eq)]
pub enum PathAccessError<'a> {
    /// A path string that could not be parsed.
    /// See [`ParseError`] for details.
    ParseError(ParseError<'a>),
    /// Access failed after parsing.
    /// See [`AccessError`] for details.
    AccessError(AccessError<'a>),
    /// An error that occurs when a type cannot downcast to a given type.
    InvalidDowncast,
}

impl fmt::Display for PathAccessError<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParseError(err) => fmt::Display::fmt(err, f),
            Self::AccessError(err) => fmt::Display::fmt(err, f),
            Self::InvalidDowncast => {
                f.write_str("Can't downcast result of access to the given type")
            }
        }
    }
}

impl core::error::Error for PathAccessError<'_> {}

impl<'a> From<ParseError<'a>> for PathAccessError<'a> {
    #[inline]
    fn from(value: ParseError<'a>) -> Self {
        Self::ParseError(value)
    }
}

impl<'a> From<AccessError<'a>> for PathAccessError<'a> {
    #[inline]
    fn from(value: AccessError<'a>) -> Self {
        Self::AccessError(value)
    }
}

// -----------------------------------------------------------------------------
// Reusable Multi-layer accessor

/// Reusable path accessor, a thin wrapper over `Box<[OffsetAccessor]>`.
///
/// [`OffsetAccessor`] and [`Accessor`](super::Accessor) only allow access to a single level,
/// while this type allows for complete path queries.
///
/// Unlike [`ReflectPathAccess`], this container parses the path string only once during initialization.
/// However, for non-static strings, it requires copying for storage.
///
/// # Examples
///
/// ```
/// use voker_reflect::{Reflect, access::PathAccessor};
///
/// #[derive(Reflect)]
/// struct Foo {
///     id: u32,
///     data: (Vec<u8>, bool),
/// }
///
/// let mut foo = Foo {
///     id: 1,
///     data: (
///         vec![1, 2, 3, 4, 5, 6, 7, 8],
///         true,
///     ),
/// };
///
/// let accessor = PathAccessor::parse_static(".data.0[3]").unwrap();
/// let val = accessor.access_as::<u8>(&foo).unwrap();
/// assert_eq!(*val, 4);
///
/// foo.data.0 = vec![10, 11, 12, 13];
/// let val = accessor.access_as::<u8>(&foo).unwrap();
/// assert_eq!(*val, 13);
/// ```
///
/// [`ReflectPathAccess`]: crate::access::ReflectPathAccess
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PathAccessor(Box<[OffsetAccessor<'static>]>);

impl From<Box<[OffsetAccessor<'static>]>> for PathAccessor {
    #[inline]
    fn from(value: Box<[OffsetAccessor<'static>]>) -> Self {
        Self(value)
    }
}

impl PathAccessor {
    /// Parses the path string and creates a [`PathAccessor`].
    ///
    /// Returns [`ParseError`] if parsing fails.
    ///
    /// This function will create a [`String`] for each path segment.
    /// For `&'static str` or `impl AccessPath<'static>`,
    /// consider using [`parse_static`] for better performance.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::access::PathAccessor;
    /// let s = String::from(".field.1#2[3]");
    /// let accessor = PathAccessor::parse(&s as &str).unwrap();
    /// ```
    ///
    /// [`String`]: alloc::string::String
    /// [`parse_static`]: crate::access::PathAccessor::parse_static
    pub fn parse<'a>(path: impl AccessPath<'a>) -> Result<Self, ParseError<'a>> {
        let mut vec: FastVec<OffsetAccessor, 4> = FastVec::new();
        let data = vec.data();

        for res in path.parse_to_accessor() {
            data.push(res?.into_owned());
        }

        Ok(Self(vec.into_boxed_slice()))
    }

    /// Parses the path and creates a [`PathAccessor`].
    ///
    /// Returns [`ParseError`] if parsing fails.
    ///
    /// For `&'static str` or `impl AccessPath<'static>`; stores string references without
    /// creating additional [`String`]s.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::access::PathAccessor;
    /// let accessor = PathAccessor::parse_static(".field.1#2[3]").unwrap();
    /// ```
    ///
    /// [`String`]: alloc::string::String
    pub fn parse_static(path: impl AccessPath<'static>) -> Result<Self, ParseError<'static>> {
        let mut vec: FastVec<OffsetAccessor, 4> = FastVec::new();
        let data = vec.data();

        for res in path.parse_to_accessor() {
            data.push(res?);
        }

        Ok(Self(vec.into_boxed_slice()))
    }

    /// Returns the length of the internal [`OffsetAccessor`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::access::PathAccessor;
    /// let accessor = PathAccessor::parse_static(".data.0[3]").unwrap();
    /// assert_eq!(accessor.len(), 3);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return `true` if the path is empty.
    ///
    /// At this point, path access directly returns the input value.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::access::PathAccessor;
    /// let accessor = PathAccessor::parse_static("").unwrap();
    /// assert!(accessor.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns a reference to the value specified by `path`.
    ///
    /// The accessor itself will not change and can be reused.
    pub fn access<'r>(
        &self,
        base: &'r dyn Reflect,
    ) -> Result<&'r dyn Reflect, PathAccessError<'static>> {
        let mut it = base;
        for accessor in &self.0 {
            it = match accessor.access(it) {
                Ok(val) => val,
                Err(err) => return Err(PathAccessError::AccessError(err)),
            };
        }
        Ok(it)
    }

    /// Returns a mutable reference to the value specified by `path`.
    ///
    /// The accessor itself will not change and can be reused.
    pub fn access_mut<'r>(
        &self,
        base: &'r mut dyn Reflect,
    ) -> Result<&'r mut dyn Reflect, PathAccessError<'static>> {
        let mut it = base;
        for accessor in &self.0 {
            it = match accessor.access_mut(it) {
                Ok(val) => val,
                Err(err) => return Err(PathAccessError::AccessError(err)),
            };
        }
        Ok(it)
    }

    /// Returns a typed reference to the value specified by `path`.
    ///
    /// The accessor itself will not change and can be reused.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::access::PathAccessor;
    /// let mut foo = (vec![1_i32, 2, 3], 1);
    /// let accessor = PathAccessor::parse_static(".0[1]").unwrap();
    ///
    /// let val = accessor.access_as::<i32>(&foo).unwrap();
    /// assert_eq!(*val, 2);
    /// ```
    #[inline]
    pub fn access_as<'r, T: Reflect>(
        &self,
        base: &'r dyn Reflect,
    ) -> Result<&'r T, PathAccessError<'static>> {
        let res = self.access(base)?;
        match res.downcast_ref::<T>() {
            Some(val) => Ok(val),
            None => Err(PathAccessError::InvalidDowncast),
        }
    }

    /// Returns a mutable typed reference to the value specified by `path`.
    ///
    /// The accessor itself will not change and can be reused.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::access::PathAccessor;
    /// let mut foo = (vec![1_i32, 2, 3], 1);
    /// let accessor = PathAccessor::parse_static(".0[1]").unwrap();
    ///
    /// let val = accessor.access_mut_as::<i32>(&mut foo).unwrap();
    /// *val += 2;
    /// assert_eq!(foo.0[1], 4);
    /// ```
    #[inline]
    pub fn access_mut_as<'r, T: Reflect>(
        &self,
        base: &'r mut dyn Reflect,
    ) -> Result<&'r mut T, PathAccessError<'static>> {
        let res = self.access_mut(base)?;
        match res.downcast_mut::<T>() {
            Some(val) => Ok(val),
            None => Err(PathAccessError::InvalidDowncast),
        }
    }

    /// Concat two `PathAccessor`.
    ///
    /// Note that this will not modify the `offset`,
    /// so the error message may not be as expected.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::access::PathAccessor;
    /// let a1 = PathAccessor::parse_static(".0[1]").unwrap();
    /// let a2 = PathAccessor::parse_static(".2").unwrap();
    /// let a = a1.concat(a2);
    /// assert_eq!(a.len(), 3);
    /// ```
    pub fn concat(self, other: PathAccessor) -> Self {
        let mut vec: Vec<OffsetAccessor> = Vec::with_capacity(self.len() + other.len());
        vec.extend(self.0);
        vec.extend(other.0);
        Self(vec.into_boxed_slice())
    }
}

impl fmt::Display for PathAccessor {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for it in &self.0 {
            fmt::Display::fmt(&it.accessor, f)?;
        }
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// Trait for once multi-layer accessing

/// Provide a single full path access method.
///
/// This will parse the path during access, and even if
/// it is not a static string, it does not need to be copied to a `String`.
///
/// If a path needs to be reused, consider using [`PathAccessor`], which only needs to be parsed once.
///
/// # Examples
///
/// ```
/// use voker_reflect::{Reflect, access::ReflectPathAccess};
///
/// #[derive(Reflect)]
/// struct Foo {
///     id: u32,
///     data: (Vec<u8>, bool),
/// }
///
/// let mut foo = Foo {
///     id: 1,
///     data: (
///         vec![1, 2, 3, 4, 5, 6, 7, 8],
///         true,
///     ),
/// };
///
/// let val = foo.access_as::<u8>(".data.0[1]").unwrap();
/// assert_eq!(*val, 2);
///
/// let val = foo.access_as::<bool>(".data.1").unwrap();
/// assert_eq!(*val, true);
///
/// // Parse errors and traversal errors are surfaced through PathAccessError.
/// let bad = foo.access_as::<u8>(".data[").unwrap_err();
/// assert!(matches!(bad, voker_reflect::access::PathAccessError::ParseError(_)));
/// ```
pub trait ReflectPathAccess {
    /// Returns a reference to the value specified by `path`.
    ///
    /// See [`ReflectPathAccess`]
    fn access<'a, 'b>(
        &'a self,
        path: impl AccessPath<'b>,
    ) -> Result<&'a dyn Reflect, PathAccessError<'b>>;

    /// Returns a mutable reference to the value specified by `path`.
    ///
    /// See [`ReflectPathAccess`]
    fn access_mut<'a, 'b>(
        &'a mut self,
        path: impl AccessPath<'b>,
    ) -> Result<&'a mut dyn Reflect, PathAccessError<'b>>;

    /// Returns a typed reference to the value specified by `path`.
    ///
    /// See [`ReflectPathAccess`]
    fn access_as<'a, 'b, T: Reflect>(
        &'a self,
        path: impl AccessPath<'b>,
    ) -> Result<&'a T, PathAccessError<'b>>;

    /// Returns a mutable typed reference to the value specified by `path`.
    ///
    /// See [`ReflectPathAccess`]
    fn access_mut_as<'a, 'b, T: Reflect>(
        &'a mut self,
        path: impl AccessPath<'b>,
    ) -> Result<&'a mut T, PathAccessError<'b>>;
}

impl ReflectPathAccess for dyn Reflect {
    #[inline(never)]
    fn access<'a, 'b>(
        &'a self,
        path: impl AccessPath<'b>,
    ) -> Result<&'a dyn Reflect, PathAccessError<'b>> {
        let mut it: &dyn Reflect = self;
        for res in path.parse_to_accessor() {
            let accessor = res?;
            it = accessor.access(it)?;
        }
        Ok(it)
    }

    #[inline(never)]
    fn access_mut<'a, 'b>(
        &'a mut self,
        path: impl AccessPath<'b>,
    ) -> Result<&'a mut dyn Reflect, PathAccessError<'b>> {
        let mut it: &mut dyn Reflect = self;
        for res in path.parse_to_accessor() {
            let accessor = res?;
            it = accessor.access_mut(it)?;
        }
        Ok(it)
    }

    #[inline]
    fn access_as<'a, 'b, T: Reflect>(
        &'a self,
        path: impl AccessPath<'b>,
    ) -> Result<&'a T, PathAccessError<'b>> {
        // Not Inline `access`: Reduce compilation time.
        // Now `access` is compiled only once per impl, independent of T.
        let it = ReflectPathAccess::access(self, path)?;
        match it.downcast_ref::<T>() {
            Some(it) => Ok(it),
            None => Err(PathAccessError::InvalidDowncast),
        }
    }

    #[inline]
    fn access_mut_as<'a, 'b, T: Reflect>(
        &'a mut self,
        path: impl AccessPath<'b>,
    ) -> Result<&'a mut T, PathAccessError<'b>> {
        // Not Inline `access`: Reduce compilation time.
        // Now `access` is compiled only once per impl, independent of T.
        let it = ReflectPathAccess::access_mut(self, path)?;
        match it.downcast_mut::<T>() {
            Some(it) => Ok(it),
            None => Err(PathAccessError::InvalidDowncast),
        }
    }
}

// -----------------------------------------------------------------------------
// Implemention for reflect type

macro_rules! impl_reflect_path_access {
    () => {
        #[inline(always)]
        fn access<'a, 'b>(
            &'a self,
            path: impl AccessPath<'b>,
        ) -> Result<&'a dyn Reflect, PathAccessError<'b>> {
            // Significantly reduce compilation time
            <dyn Reflect as ReflectPathAccess>::access(self, path)
        }

        #[inline(always)]
        fn access_mut<'a, 'b>(
            &'a mut self,
            path: impl AccessPath<'b>,
        ) -> Result<&'a mut dyn Reflect, PathAccessError<'b>> {
            // Significantly reduce compilation time
            <dyn Reflect as ReflectPathAccess>::access_mut(self, path)
        }

        #[inline(always)]
        fn access_as<'a, 'b, T: Reflect>(
            &'a self,
            path: impl AccessPath<'b>,
        ) -> Result<&'a T, PathAccessError<'b>> {
            // Significantly reduce compilation time
            <dyn Reflect as ReflectPathAccess>::access_as::<T>(self, path)
        }

        #[inline(always)]
        fn access_mut_as<'a, 'b, T: Reflect>(
            &'a mut self,
            path: impl AccessPath<'b>,
        ) -> Result<&'a mut T, PathAccessError<'b>> {
            // Significantly reduce compilation time
            <dyn Reflect as ReflectPathAccess>::access_mut_as::<T>(self, path)
        }
    };
    (dyn $name:ident) => {
        impl ReflectPathAccess for dyn $name {
            impl_reflect_path_access!();
        }
    };
    (T: $name:ident) => {
        impl<P: Sized + $name> ReflectPathAccess for P {
            impl_reflect_path_access!();
        }
    };
}

impl_reflect_path_access!(T: Reflect);

impl_reflect_path_access!(dyn Struct);
impl_reflect_path_access!(dyn TupleStruct);
impl_reflect_path_access!(dyn Tuple);
impl_reflect_path_access!(dyn List);
impl_reflect_path_access!(dyn Array);
impl_reflect_path_access!(dyn Enum);

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use super::{PathAccessError, PathAccessor, ReflectPathAccess};
    use crate::Reflect;
    use alloc::string::ToString;
    use alloc::vec;
    use alloc::vec::Vec;

    #[derive(Reflect)]
    struct Inner {
        value: i32,
    }

    #[derive(Reflect)]
    struct Outer {
        inner: Inner,
        values: Vec<i32>,
    }

    #[test]
    fn access_mut() {
        let accessor = PathAccessor::parse_static(".inner.value").unwrap();
        let mut value = Outer {
            inner: Inner { value: 1 },
            values: vec![10, 20, 30],
        };

        assert_eq!(accessor.to_string(), ".inner.value");
        *accessor.access_mut_as::<i32>(&mut value).unwrap() = 5;

        assert_eq!(value.inner.value, 5);
        assert_eq!(
            *PathAccessor::parse_static(".values[1]")
                .unwrap()
                .access_as::<i32>(&value)
                .unwrap(),
            20
        );
    }

    #[test]
    fn parse_errors() {
        let value = Outer {
            inner: Inner { value: 7 },
            values: vec![1, 2, 3],
        };

        let err = PathAccessor::parse_static(".inner.value")
            .unwrap()
            .access_as::<bool>(&value)
            .unwrap_err();
        assert_eq!(err, PathAccessError::InvalidDowncast);

        let err = value.access_as::<i32>(".values[").unwrap_err();
        assert!(matches!(err, PathAccessError::ParseError(_)));
    }
}
