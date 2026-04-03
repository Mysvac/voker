use alloc::{boxed::Box, vec::Vec};
use core::cmp::Ordering;
use core::fmt;
use core::iter::FusedIterator;

use super::{impl_dynamic_type_info, impl_dynamic_type_path};
use crate::Reflect;
use crate::info::TypeInfo;
use crate::ops::{ApplyError, DynamicTupleStruct, ReflectCloneError};
use crate::reflection::impl_reflect_cast_fn;

// -----------------------------------------------------------------------------
// Dynamic Tuple

/// A dynamic container representing a tuple-like collection.
///
/// `DynamicTuple` is a type-erased dynamic tuple that can hold any types implementing
/// [`Reflect`]. Unlike static Rust tuples (`(T, U, V)`), `DynamicTuple` can change its
/// length dynamically using [`extend`] or [`extend_boxed`].
///
/// # Type Information
///
/// Dynamic types are special in that their `TypeInfo` is [`OpaqueInfo`](crate::info::OpaqueInfo),
/// but other APIs behave like the represented type, such as [`reflect_kind`] and [`reflect_ref`].
///
/// A `DynamicTuple` can optionally represent a specific tuple type through its
/// [`represented_type_info`]. When set, this allows the dynamic tuple to be treated
/// as if it were a specific static tuple type for reflection purposes.
///
/// But remember, we do not check whether the number and type of elements inside
/// the container are correct, and users need to pay attention to it.
///
/// # Examples
///
/// ## Creating and extending a dynamic tuple
///
/// ```
/// use voker_reflect::ops::{DynamicTuple, Tuple};
///
/// let mut dynamic = DynamicTuple::new();
/// dynamic.extend(1_i32);
/// dynamic.extend("hello");
/// dynamic.extend(true);
///
/// assert_eq!(dynamic.field_len(), 3);
/// ```
///
/// ## Converting from an iterator
///
/// ```
/// use voker_reflect::ops::{DynamicTuple, Tuple};
/// use voker_reflect::Reflect;
///
/// let fields = vec![
///     Box::new(1_i32) as Box<dyn Reflect>,
///     Box::new("world"),
///     Box::new(3.14_f64),
/// ];
///
/// let dynamic: DynamicTuple = fields.into_iter().collect();
/// assert_eq!(dynamic.field_len(), 3);
/// ```
///
/// ## Applying to a static tuple
///
/// ```
/// use voker_reflect::{Reflect, ops::{Tuple, DynamicTuple}};
///
/// let mut dynamic = DynamicTuple::new();
/// dynamic.extend(10);
/// dynamic.extend(20);
/// dynamic.extend(30);
///
/// let mut tup = (0, 0, 0);
/// tup.apply(&dynamic);
///
/// assert_eq!(tup, (10, 20, 30));
/// ```
///
/// [`reflect_kind`]: Reflect::reflect_kind
/// [`reflect_ref`]: Reflect::reflect_ref
/// [`extend`]: DynamicTuple::extend
/// [`extend_boxed`]: DynamicTuple::extend_boxed
/// [`represented_type_info`]: Reflect::represented_type_info
#[derive(Default)]
pub struct DynamicTuple {
    info: Option<&'static TypeInfo>,
    fields: Vec<Box<dyn Reflect>>,
}

impl_dynamic_type_path!(DynamicTuple);
impl_dynamic_type_info!(DynamicTuple);

impl DynamicTuple {
    /// Creates an empty `DynamicTuple`.
    ///
    /// If you already have data to populate the tuple, consider using
    /// [`FromIterator::from_iter`] instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::ops::DynamicTuple;
    /// let dynamic = DynamicTuple::new();
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self {
            info: None,
            fields: Vec::new(),
        }
    }

    /// Creates a new empty `DynamicTuple` with at least the specified capacity.
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

    /// Sets the [`TypeInfo`] that this dynamic tuple represents.
    ///
    /// When set, [`Reflect::represented_type_info`] will return this information,
    /// allowing the dynamic tuple to be treated as if it were a specific static tuple type.
    ///
    /// # Panics
    ///
    /// Panics if `info` is `Some` but does not contain tuple type information.
    pub const fn set_type_info(&mut self, info: Option<&'static TypeInfo>) {
        match info {
            Some(info) => {
                assert!(info.kind().is_tuple(), "`TypeInfo` mismatched.");
                self.info = Some(info);
            }
            None => {
                self.info = None;
            }
        }
    }

    /// Appends a boxed [`Reflect`] value to the end of the tuple.
    ///
    /// This is the low-level version of [`extend`] that accepts already-boxed values.
    ///
    /// # Note
    ///
    /// While tuples in Rust have fixed lengths and specific types for each position,
    /// `DynamicTuple` can be extended dynamically and can hold heterogeneous types.
    ///
    /// This makes it useful for building up tuples before applying them to static tuples
    /// of known size and type.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::ops::{Tuple, DynamicTuple};
    ///
    /// let mut dynamic = DynamicTuple::new();
    /// dynamic.extend_boxed(Box::new(1_i32));
    /// dynamic.extend_boxed(Box::new("hello"));
    /// dynamic.extend_boxed(Box::new(true));
    ///
    /// assert_eq!(dynamic.field_len(), 3);
    /// ```
    ///
    /// [`extend`]: DynamicTuple::extend
    pub fn extend_boxed(&mut self, value: Box<dyn Reflect>) {
        self.fields.push(value);
    }

    /// Appends a value to the end of the tuple.
    ///
    /// This is a convenience method that boxes the value and calls
    /// [`extend_boxed`].
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::ops::{Tuple, DynamicTuple};
    ///
    /// let mut dynamic = DynamicTuple::new();
    /// dynamic.extend(42_i32);
    /// dynamic.extend("world");
    /// dynamic.extend(3.14_f64);
    ///
    /// assert_eq!(dynamic.field_len(), 3);
    /// ```
    ///
    /// [`extend_boxed`]: DynamicTuple::extend_boxed
    #[inline]
    pub fn extend<T: Reflect>(&mut self, value: T) {
        self.extend_boxed(Box::new(value));
    }
}

impl Reflect for DynamicTuple {
    impl_reflect_cast_fn!(Tuple);

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
        Box::new(<Self as Tuple>::to_dynamic_tuple(self))
    }

    #[inline]
    fn reflect_clone(&self) -> Result<Box<dyn Reflect>, ReflectCloneError> {
        Ok(Box::new(<Self as Tuple>::to_dynamic_tuple(self)))
    }

    #[inline]
    fn apply(&mut self, value: &dyn Reflect) -> Result<(), ApplyError> {
        crate::impls::tuple_apply(self, value)
    }

    #[inline]
    fn reflect_hash(&self) -> Option<u64> {
        crate::impls::tuple_hash(self)
    }

    #[inline]
    fn reflect_eq(&self, other: &dyn Reflect) -> Option<bool> {
        crate::impls::tuple_eq(self, other)
    }

    #[inline]
    fn reflect_cmp(&self, other: &dyn Reflect) -> Option<Ordering> {
        crate::impls::tuple_cmp(self, other)
    }

    #[inline]
    fn reflect_debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DynamicTuple(")?;
        crate::impls::tuple_debug(self, f)?;
        write!(f, ")")
    }
}

impl fmt::Debug for DynamicTuple {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.reflect_debug(f)
    }
}

impl FromIterator<Box<dyn Reflect>> for DynamicTuple {
    fn from_iter<I: IntoIterator<Item = Box<dyn Reflect>>>(fields: I) -> Self {
        Self {
            info: None,
            fields: fields.into_iter().collect(),
        }
    }
}

impl From<DynamicTuple> for DynamicTupleStruct {
    fn from(value: DynamicTuple) -> Self {
        DynamicTupleStruct {
            info: None,
            fields: value.fields,
        }
    }
}

impl From<DynamicTupleStruct> for DynamicTuple {
    fn from(value: DynamicTupleStruct) -> Self {
        DynamicTuple {
            info: None,
            fields: value.fields,
        }
    }
}

impl IntoIterator for DynamicTuple {
    type Item = Box<dyn Reflect>;
    type IntoIter = alloc::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.fields.into_iter()
    }
}

impl<'a> IntoIterator for &'a DynamicTuple {
    type Item = &'a dyn Reflect;
    type IntoIter = TupleFieldIter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_fields()
    }
}

// -----------------------------------------------------------------------------
// Tuple trait

/// A trait for type-erased tuple-like operations via reflection.
///
/// This trait represents any fixed-size heterogeneous collection, including:
/// - Rust tuples (`(T, U, V)`)
/// - Types that can be viewed as tuples through reflection
///
/// This trait is automatically implemented for arbitrary tuples of up to **12**
/// elements, provided that each element implements [`Reflect`].
///
/// # Contract
///
/// Implementors must maintain a fixed number of fields as returned by [`Tuple::field_len`].
/// Unlike arrays, tuples can contain elements of different types, though in practice
/// implementors typically know the specific type of each field position.
///
/// # Examples
///
/// ## Using with static tuples
///
/// ```
/// use voker_reflect::ops::Tuple;
///
/// let tuple = (10_u32, "hello", true);
/// let tuple_ref: &dyn Tuple = &tuple;
///
/// assert_eq!(tuple_ref.field_len(), 3);
/// assert_eq!(tuple_ref.field_as::<u32>(0), Some(&10));
/// assert_eq!(tuple_ref.field_as::<&str>(1), Some(&"hello"));
/// assert_eq!(tuple_ref.field_as::<bool>(2), Some(&true));
/// ```
///
/// ## Iterating over tuple fields
///
/// ```
/// use voker_reflect::{Reflect, ops::Tuple};
///
/// let tuple = (1, "test", 3.14);
/// let tuple_ref: &dyn Tuple = &tuple;
///
/// let fields: Vec<&dyn Reflect> = tuple_ref.iter_fields().collect();
/// assert_eq!(fields.len(), 3);
/// ```
///
/// [`field_len`]: Tuple::field_len
/// [`field`]: Tuple::field
/// [`field_mut`]: Tuple::field_mut
pub trait Tuple: Reflect {
    /// Returns a reference to the value of the field with index `index` as a
    /// `&dyn Reflect`.
    ///
    /// Returns `None` if `index` is out of bounds.
    ///
    /// For type-safe access when the field type is known, use `<dyn Tuple>::field_as` instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::ops::Tuple;
    /// let tuple = (1, "hello", true);
    ///
    /// assert!(tuple.field(0).is_some());
    /// assert!(tuple.field(3).is_none());
    /// ```
    fn field(&self, index: usize) -> Option<&dyn Reflect>;

    /// Returns a mutable reference to the value of the field with index `index`
    /// as a `&mut dyn Reflect`.
    ///
    /// Returns `None` if `index` is out of bounds.
    ///
    /// For type-safe mutable access when the field type is known,
    /// use `<dyn Tuple>::field_mut_as` instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::Tuple};
    /// let mut tuple = (1_i32, "test");
    ///
    /// if let Some(field) = tuple.field_mut(0) {
    ///     *field.downcast_mut::<i32>().unwrap() = 42;
    /// }
    ///
    /// assert_eq!(tuple.0, 42);
    /// ```
    fn field_mut(&mut self, index: usize) -> Option<&mut dyn Reflect>;

    /// Returns the number of fields in the tuple.
    ///
    /// # Contract
    ///
    /// This value must remain constant for the lifetime of the object.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::ops::Tuple;
    /// let tuple = (1, 2, 3, 4, 5);
    /// assert_eq!(tuple.field_len(), 5);
    /// ```
    fn field_len(&self) -> usize;

    /// Returns an iterator over the values of the tuple's fields.
    ///
    /// The iterator yields references to each field in order,
    /// from index 0 to `field_len() - 1`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::Tuple};
    /// let tuple = (10, "hello", 30);
    ///
    /// let mut values = vec![];
    /// for field in tuple.iter_fields() {
    ///     values.push(field);
    /// }
    ///
    /// assert_eq!(values.len(), 3);
    /// ```
    fn iter_fields(&self) -> TupleFieldIter<'_>;

    /// Consumes the boxed tuple and returns its fields as a vector.
    ///
    /// This is useful when you need to take ownership of the tuple's contents.
    fn drain(self: Box<Self>) -> Vec<Box<dyn Reflect>>;

    /// Creates a [`DynamicTuple`] copy of this tuple.
    ///
    /// This is useful when you need a mutable, resizable version of a static tuple.
    ///
    /// This function will replace all content with dynamic types, except for opaque types.
    ///
    /// # Panics
    ///
    /// Panics if inner items [`Reflect::to_dynamic`] failed.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::ops::{DynamicTuple, Tuple};
    ///
    /// let tuple = (1, "hello", true);
    /// let dynamic: DynamicTuple = tuple.to_dynamic_tuple();
    /// assert_eq!(dynamic.field_len(), 3);
    /// ```
    fn to_dynamic_tuple(&self) -> DynamicTuple {
        DynamicTuple {
            info: self.represented_type_info(),
            fields: self.iter_fields().map(Reflect::to_dynamic).collect(),
        }
    }
}

impl Tuple for DynamicTuple {
    #[inline]
    fn field(&self, index: usize) -> Option<&dyn Reflect> {
        self.fields.get(index).map(core::ops::Deref::deref)
    }

    #[inline]
    fn field_mut(&mut self, index: usize) -> Option<&mut dyn Reflect> {
        self.fields.get_mut(index).map(core::ops::DerefMut::deref_mut)
    }

    #[inline]
    fn field_len(&self) -> usize {
        self.fields.len()
    }

    #[inline]
    fn iter_fields(&self) -> TupleFieldIter<'_> {
        TupleFieldIter::new(self)
    }

    #[inline]
    fn drain(self: Box<Self>) -> Vec<Box<dyn Reflect>> {
        self.fields
    }
}

impl dyn Tuple {
    /// Returns a typed reference to the field at the given index.
    ///
    /// Returns `None` if:
    /// - The index is out of bounds
    /// - The field cannot be downcast to type `T`
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::ops::Tuple;
    /// let tuple = (10_i32, "hello", true);
    /// let tuple_ref: &dyn Tuple = &tuple;
    ///
    /// assert_eq!(tuple_ref.field_as::<i32>(0), Some(&10));
    /// assert_eq!(tuple_ref.field_as::<&str>(1), Some(&"hello"));
    /// # assert_eq!(tuple_ref.field_as::<bool>(2), Some(&true));
    /// assert_eq!(tuple_ref.field_as::<i32>(3), None); // Out of bounds
    /// assert_eq!(tuple_ref.field_as::<f64>(0), None); // Wrong type
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
    /// # use voker_reflect::ops::Tuple;
    /// let mut tuple = (10_i32, "hello");
    /// let tuple_ref: &mut dyn Tuple = &mut tuple;
    ///
    /// if let Some(field) = tuple_ref.field_mut_as::<i32>(0) {
    ///     *field = 42;
    /// }
    ///
    /// assert_eq!(tuple.0, 42);
    /// ```
    #[inline]
    pub fn field_mut_as<T: Reflect>(&mut self, index: usize) -> Option<&mut T> {
        self.field_mut(index).and_then(<dyn Reflect>::downcast_mut)
    }
}

// -----------------------------------------------------------------------------
// Tuple Field Iterator

/// An iterator over the field values of a tuple.
///
/// This is an [`ExactSizeIterator`] that yields references to each field
/// in the tuple in order.
///
/// # Performance
///
/// The iterator uses [`Tuple::field`] internally, which may have different
/// performance characteristics than iterating directly over a concrete tuple type.
///
/// # Examples
///
/// ```
/// use voker_reflect::{Reflect, ops::{Tuple, TupleFieldIter}};
///
/// let tuple = (1, "test", true);
/// let mut iter = TupleFieldIter::new(&tuple);
///
/// assert_eq!(iter.len(), 3);
/// assert_eq!(iter.next().and_then(|v| v.downcast_ref::<i32>()), Some(&1));
/// ```
pub struct TupleFieldIter<'a> {
    tuple: &'a dyn Tuple,
    index: usize,
}

impl<'a> TupleFieldIter<'a> {
    /// Creates a new iterator for the given tuple.
    #[inline(always)]
    pub const fn new(value: &'a dyn Tuple) -> Self {
        TupleFieldIter {
            tuple: value,
            index: 0,
        }
    }
}

impl<'a> Iterator for TupleFieldIter<'a> {
    type Item = &'a dyn Reflect;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let value = self.tuple.field(self.index);
        self.index += value.is_some() as usize;
        value
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let hint = self.tuple.field_len() - self.index;
        (hint, Some(hint))
    }
}

impl ExactSizeIterator for TupleFieldIter<'_> {}
impl FusedIterator for TupleFieldIter<'_> {}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use super::DynamicTuple;
    use crate::info::TypePath;

    #[test]
    fn type_path() {
        assert!(DynamicTuple::type_path() == "voker_reflect::ops::DynamicTuple");
        assert!(DynamicTuple::module_path() == Some("voker_reflect::ops"));
        assert!(DynamicTuple::type_ident() == "DynamicTuple");
        assert!(DynamicTuple::type_name() == "DynamicTuple");
    }
}
