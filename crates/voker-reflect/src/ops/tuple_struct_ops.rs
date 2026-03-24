use alloc::{boxed::Box, vec::Vec};
use core::cmp::Ordering;
use core::fmt;
use core::iter::FusedIterator;

use crate::Reflect;
use crate::impls::NonGenericTypeInfoCell;
use crate::info::{OpaqueInfo, TypeInfo, TypePath, Typed};
use crate::ops::{ApplyError, ReflectCloneError};
use crate::reflection::impl_reflect_cast_fn;

// -----------------------------------------------------------------------------
// Dynamic TupleStruct

/// A dynamic container representing a tuple-struct.
///
/// `DynamicTupleStruct` is a type-erased dynamic tuple-struct that can hold any types
/// implementing [`Reflect`].
///
/// `DynamicTupleStruct` can change its fields dynamically using [`extend`] or [`extend_boxed`].
///
/// # Type Information
///
/// Dynamic types are special in that their `TypeInfo` is [`OpaqueInfo`],
/// but other APIs behave like the represented type, such as [`reflect_kind`] and [`reflect_ref`].
///
/// A `DynamicTupleStruct` can optionally represent a specific tuple-struct type through its
/// [`represented_type_info`]. When set, this allows the dynamic tuple-struct to be treated
/// as if it were a specific static tuple-struct type for reflection purposes.
///
/// But remember, we do not check whether the number and type of elements inside
/// the container are correct, and users need to pay attention to it.
///
/// # Examples
///
/// ## Creating and extending a dynamic tuple-struct
///
/// ```
/// use voker_reflect::ops::{DynamicTupleStruct, TupleStruct};
///
/// let mut dynamic = DynamicTupleStruct::new();
/// dynamic.extend(1_i32);
/// dynamic.extend("hello");
/// dynamic.extend(true);
///
/// assert_eq!(dynamic.field_len(), 3);
/// ```
///
/// ## Applying to a static tuple-struct
///
/// ```
/// use voker_reflect::{Reflect, ops::{TupleStruct, DynamicTupleStruct}};
///
/// #[derive(Reflect, PartialEq, Debug)]
/// struct Foo(i32, i64, bool);
///
/// let mut dynamic = DynamicTupleStruct::new();
/// dynamic.extend(10_i32);
/// dynamic.extend(20_i64);
/// dynamic.extend(true);
///
/// let mut tup = Foo(0_i32, 0_i64, false);
/// tup.apply(&dynamic);
///
/// assert_eq!(tup, Foo(10_i32, 20_i64, true));
/// ```
///
/// [`reflect_kind`]: Reflect::reflect_kind
/// [`reflect_ref`]: Reflect::reflect_ref
/// [`extend`]: DynamicTupleStruct::extend
/// [`extend_boxed`]: DynamicTupleStruct::extend_boxed
/// [`represented_type_info`]: Reflect::represented_type_info
#[derive(Default)]
pub struct DynamicTupleStruct {
    pub(super) info: Option<&'static TypeInfo>,
    pub(super) fields: Vec<Box<dyn Reflect>>,
}

// Explicitly implemented here so that code readers do not need
// to ponder the principles of proc-macros in advance.
impl TypePath for DynamicTupleStruct {
    #[inline]
    fn type_path() -> &'static str {
        "voker_reflect::ops::DynamicTupleStruct"
    }

    #[inline]
    fn type_name() -> &'static str {
        "DynamicTupleStruct"
    }

    #[inline]
    fn type_ident() -> &'static str {
        "DynamicTupleStruct"
    }

    #[inline]
    fn module_path() -> Option<&'static str> {
        Some("voker_reflect::ops")
    }
}

impl Typed for DynamicTupleStruct {
    fn type_info() -> &'static TypeInfo {
        static CELL: NonGenericTypeInfoCell = NonGenericTypeInfoCell::new();
        CELL.get_or_init(|| TypeInfo::Opaque(OpaqueInfo::new::<Self>()))
    }
}

impl DynamicTupleStruct {
    /// Creates an empty `DynamicTupleStruct`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::ops::DynamicTupleStruct;
    /// let dynamic = DynamicTupleStruct::new();
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self {
            info: None,
            fields: Vec::new(),
        }
    }

    /// Creates a new empty `DynamicTupleStruct` with at least the specified capacity.
    ///
    /// This can be used to avoid reallocations when you know approximately
    /// how many fields will be added to the tuple.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            info: None,
            fields: Vec::with_capacity(capacity),
        }
    }

    /// Sets the [`TypeInfo`] that this dynamic tuple-struct represents.
    ///
    /// When set, [`Reflect::represented_type_info`] will return this information,
    /// allowing the dynamic tuple-struct to be treated as if it were a specific static tuple-struct type.
    ///
    /// # Panics
    ///
    /// Panics if `info` is `Some` but does not contain tuple-struct type information.
    pub const fn set_type_info(&mut self, info: Option<&'static TypeInfo>) {
        match info {
            Some(info) => {
                assert!(info.is_tuple_struct(), "`TypeInfo` mismatched.");
                self.info = Some(info);
            }
            None => {
                self.info = None;
            }
        }
    }

    /// Appends a boxed [`Reflect`] value to the end of the tuple-struct.
    ///
    /// This is the low-level version of [`extend`] that accepts already-boxed values.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::ops::{TupleStruct, DynamicTupleStruct};
    ///
    /// let mut dynamic = DynamicTupleStruct::new();
    /// dynamic.extend_boxed(Box::new(1_i32));
    /// dynamic.extend_boxed(Box::new("hello"));
    /// dynamic.extend_boxed(Box::new(true));
    ///
    /// assert_eq!(dynamic.field_len(), 3);
    /// ```
    ///
    /// [`extend`]: DynamicTupleStruct::extend
    pub fn extend_boxed(&mut self, value: Box<dyn Reflect>) {
        self.fields.push(value);
    }

    /// Appends a value to the end of the tuple-struct.
    ///
    /// This is a convenience method that boxes the value and calls
    /// [`extend_boxed`].
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::ops::{TupleStruct, DynamicTupleStruct};
    ///
    /// let mut dynamic = DynamicTupleStruct::new();
    /// dynamic.extend(42_i32);
    /// dynamic.extend("world");
    /// dynamic.extend(3.14_f64);
    ///
    /// assert_eq!(dynamic.field_len(), 3);
    /// ```
    ///
    /// [`extend_boxed`]: DynamicTupleStruct::extend_boxed
    #[inline]
    pub fn extend<T: Reflect>(&mut self, value: T) {
        self.extend_boxed(Box::new(value))
    }
}

impl Reflect for DynamicTupleStruct {
    impl_reflect_cast_fn!(TupleStruct);

    #[inline]
    fn is_dynamic(&self) -> bool {
        true
    }

    #[inline]
    fn represented_type_info(&self) -> Option<&'static TypeInfo> {
        self.info
    }

    #[inline]
    fn to_dynamic(&self) -> Box<dyn Reflect> {
        Box::new(<Self as TupleStruct>::to_dynamic_tuple_struct(self))
    }

    #[inline]
    fn reflect_clone(&self) -> Result<Box<dyn Reflect>, ReflectCloneError> {
        Ok(Box::new(<Self as TupleStruct>::to_dynamic_tuple_struct(
            self,
        )))
    }

    #[inline]
    fn apply(&mut self, value: &dyn Reflect) -> Result<(), ApplyError> {
        crate::impls::tuple_struct_apply(self, value)
    }

    #[inline]
    fn reflect_eq(&self, other: &dyn Reflect) -> Option<bool> {
        crate::impls::tuple_struct_eq(self, other)
    }

    #[inline]
    fn reflect_cmp(&self, other: &dyn Reflect) -> Option<Ordering> {
        crate::impls::tuple_struct_cmp(self, other)
    }

    #[inline]
    fn reflect_hash(&self) -> Option<u64> {
        crate::impls::tuple_struct_hash(self)
    }

    #[inline]
    fn reflect_debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DynamicTupleStruct(")?;
        crate::impls::tuple_struct_debug(self, f)?;
        write!(f, ")")
    }
}

impl fmt::Debug for DynamicTupleStruct {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.reflect_debug(f)
    }
}

impl FromIterator<Box<dyn Reflect>> for DynamicTupleStruct {
    fn from_iter<T: IntoIterator<Item = Box<dyn Reflect>>>(iter: T) -> Self {
        Self {
            info: None,
            fields: iter.into_iter().collect(),
        }
    }
}

impl IntoIterator for DynamicTupleStruct {
    type Item = Box<dyn Reflect>;
    type IntoIter = alloc::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.fields.into_iter()
    }
}

impl<'a> IntoIterator for &'a DynamicTupleStruct {
    type Item = &'a dyn Reflect;
    type IntoIter = TupleStructFieldIter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_fields()
    }
}

// -----------------------------------------------------------------------------
// TupleStruct trait

/// A trait for type-erased tuple-struct operations via reflection.
///
/// This trait represents any fixed-size heterogeneous collection, including:
/// - Rust tuple-structs (e.g. `Foo(T, U, V)`)
/// - Types that can be viewed as tuple-structs through reflection
///
/// When using [`#[derive(Reflect)]`](crate::derive::Reflect) on a standard tuple-struct,
/// this trait will be automatically implemented.
///
/// # Examples
///
/// ```
/// use voker_reflect::{Reflect, ops::TupleStruct};
///
/// #[derive(Reflect)]
/// struct Foo(i32, &'static str, bool);
///
/// let ts = Foo(10_i32, "hello", true);
/// let ts_ref: &dyn TupleStruct = &ts;
///
/// assert_eq!(ts_ref.field_len(), 3);
/// assert_eq!(ts_ref.field_as::<i32>(0), Some(&10));
/// assert_eq!(ts_ref.field_as::<&str>(1), Some(&"hello"));
/// assert_eq!(ts_ref.field_as::<bool>(2), Some(&true));
/// ```
pub trait TupleStruct: Reflect {
    /// Returns a reference to the value of the field with index `index` as a
    /// `&dyn Reflect`.
    ///
    /// Returns `None` if `index` is out of bounds.
    ///
    /// If the field type is known, can use `<dyn TupleStruct>::field_as` instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::TupleStruct};
    /// #[derive(Reflect)]
    /// struct Foo(i32, &'static str, bool);
    ///
    /// let ts = Foo(1, "hello", true);
    ///
    /// assert!(ts.field(0).is_some());
    /// assert!(ts.field(3).is_none());
    /// ```
    fn field(&self, index: usize) -> Option<&dyn Reflect>;

    /// Returns a mutable reference to the value of the field with index `index`
    /// as a `&mut dyn Reflect`.
    ///
    /// Returns `None` if `index` is out of bounds.
    ///
    /// If the field type is known, can use `<dyn TupleStruct>::field_mut_as` instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::TupleStruct};
    /// #[derive(Reflect)]
    /// struct Foo(i32, &'static str);
    ///
    /// let mut ts = Foo(1_i32, "test");
    ///
    /// if let Some(field) = ts.field_mut(0) {
    ///     *field.downcast_mut::<i32>().unwrap() = 42;
    /// }
    ///
    /// assert_eq!(ts.0, 42);
    /// ```
    fn field_mut(&mut self, index: usize) -> Option<&mut dyn Reflect>;

    /// Returns the number of fields in the tuple-struct.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::TupleStruct};
    /// #[derive(Reflect)]
    /// struct Foo(i32, i32, i32);
    ///
    /// let ts = Foo(1, 2, 3);
    ///
    /// assert_eq!(ts.field_len(), 3);
    /// ```
    fn field_len(&self) -> usize;

    /// Returns an iterator over the values of the tuple-struct's fields.
    ///
    /// The iterator yields references to each field in order,
    /// from index 0 to `field_len() - 1`.
    fn iter_fields(&self) -> TupleStructFieldIter<'_>;

    /// Creates a [`DynamicTupleStruct`] copy of this tuple-struct.
    ///
    /// This is useful when you need a mutable, resizable version of a static tuple-struct.
    ///
    /// This function will replace all content with dynamic types, except for opaque types.
    ///
    /// # Panics
    ///
    /// Panics if inner items [`Reflect::to_dynamic`] failed.
    fn to_dynamic_tuple_struct(&self) -> DynamicTupleStruct {
        DynamicTupleStruct {
            info: self.represented_type_info(),
            fields: self.iter_fields().map(Reflect::to_dynamic).collect(),
        }
    }
}

impl TupleStruct for DynamicTupleStruct {
    #[inline]
    fn field(&self, index: usize) -> Option<&dyn Reflect> {
        self.fields.get(index).map(|field| &**field)
    }

    #[inline]
    fn field_mut(&mut self, index: usize) -> Option<&mut dyn Reflect> {
        self.fields.get_mut(index).map(|field| &mut **field)
    }

    #[inline]
    fn field_len(&self) -> usize {
        self.fields.len()
    }

    #[inline]
    fn iter_fields(&self) -> TupleStructFieldIter<'_> {
        TupleStructFieldIter::new(self)
    }
}

impl dyn TupleStruct {
    /// Returns a typed reference to the field at the given index.
    ///
    /// Returns `None` if:
    /// - The index is out of bounds
    /// - The field cannot be downcast to type `T`
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{ops::TupleStruct, Reflect};
    /// #[derive(Reflect)]
    /// struct Foo(i32, &'static str);
    ///
    /// let ts = Foo(10_i32, "hello");
    /// let ts_ref: &dyn TupleStruct = &ts;
    ///
    /// assert_eq!(ts_ref.field_as::<i32>(0), Some(&10));
    /// assert_eq!(ts_ref.field_as::<&str>(1), Some(&"hello"));
    /// assert_eq!(ts_ref.field_as::<i32>(2), None); // Out of bounds
    /// assert_eq!(ts_ref.field_as::<f64>(0), None); // Wrong type
    /// ```
    #[inline]
    pub fn field_as<T: Reflect>(&self, index: usize) -> Option<&T> {
        self.field(index).and_then(<dyn Reflect>::downcast_ref)
    }

    /// Returns a typed mutable reference to the field at the given index.
    ///
    /// Returns `None` if:
    /// - The index is out of bounds
    /// - The field cannot be downcast to type `T`
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{ops::TupleStruct, Reflect};
    /// #[derive(Reflect)]
    /// struct A(i32, &'static str);
    ///
    /// let mut ts = A(10_i32, "hello");
    /// let ts_ref: &mut dyn TupleStruct = &mut ts;
    ///
    /// if let Some(field) = ts_ref.field_mut_as::<i32>(0) {
    ///     *field = 31;
    /// }
    ///
    /// assert_eq!(ts.0, 31);
    /// ```
    #[inline]
    pub fn field_mut_as<T: Reflect>(&mut self, index: usize) -> Option<&mut T> {
        self.field_mut(index).and_then(<dyn Reflect>::downcast_mut)
    }
}

// -----------------------------------------------------------------------------
// TupleStruct Iterator

/// An iterator over the field values of a tuple-struct.
///
/// This is an [`ExactSizeIterator`] that yields references to each field
/// in the tuple-struct in order.
///
/// # Performance
///
/// The iterator uses [`TupleStruct::field`] internally, which may have different
/// performance characteristics than iterating directly over a concrete tuple-struct type.
///
/// # Examples
///
/// ```
/// use voker_reflect::{Reflect, ops::{TupleStruct, TupleStructFieldIter}};
///
/// #[derive(Reflect)]
/// struct Foo(i32, &'static str, bool);
///
/// let ts = Foo(1, "test", true);
/// let mut iter = TupleStructFieldIter::new(&ts);
///
/// assert_eq!(iter.len(), 3);
/// assert_eq!(iter.next().and_then(|v| v.downcast_ref::<i32>()), Some(&1));
/// ```
pub struct TupleStructFieldIter<'a> {
    tuple_struct: &'a dyn TupleStruct,
    index: usize,
}

impl<'a> TupleStructFieldIter<'a> {
    /// Creates a new iterator for the given tuple-struct.
    #[inline(always)]
    pub const fn new(value: &'a dyn TupleStruct) -> Self {
        TupleStructFieldIter {
            tuple_struct: value,
            index: 0,
        }
    }
}

impl<'a> Iterator for TupleStructFieldIter<'a> {
    type Item = &'a dyn Reflect;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let value = self.tuple_struct.field(self.index);
        self.index += value.is_some() as usize;
        value
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let hint = self.tuple_struct.field_len() - self.index;
        (hint, Some(hint))
    }
}

impl ExactSizeIterator for TupleStructFieldIter<'_> {}
impl FusedIterator for TupleStructFieldIter<'_> {}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use super::DynamicTupleStruct;
    use crate::info::TypePath;

    #[test]
    fn type_path() {
        assert!(DynamicTupleStruct::type_path() == "voker_reflect::ops::DynamicTupleStruct");
        assert!(DynamicTupleStruct::module_path() == Some("voker_reflect::ops"));
        assert!(DynamicTupleStruct::type_ident() == "DynamicTupleStruct");
        assert!(DynamicTupleStruct::type_name() == "DynamicTupleStruct");
    }
}
