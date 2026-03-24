use alloc::{boxed::Box, vec::Vec};
use core::cmp::Ordering;
use core::fmt;
use core::iter::FusedIterator;

use crate::Reflect;
use crate::impls::NonGenericTypeInfoCell;
use crate::info::{OpaqueInfo, TypeInfo, TypePath, Typed};
use crate::ops::{ApplyError, ReflectCloneError};

// -----------------------------------------------------------------------------
// Dynamic Array

/// A dynamic container representing an array-like collection.
///
/// `DynamicArray` is a type-erased dynamic array that can hold any type implementing
/// [`Reflect`]. Unlike static Rust arrays (`[T; N]`), `DynamicArray` can change its
/// length dynamically using [`extend`] or [`extend_boxed`].
///
/// # Type Information
///
/// Dynamic types are special in that their `TypeInfo` is [`OpaqueInfo`],
/// but other APIs behave like the represented type, such as [`reflect_kind`] and [`reflect_ref`].
///
/// A `DynamicArray` can optionally represent a specific array type through its
/// [`represented_type_info`]. When set, this allows the dynamic array to be treated
/// as if it were a specific static array type for reflection purposes.
///
/// But remember, we do not check whether the number and type of elements inside
/// the container are correct, and users need to pay attention to it.
///
/// # Examples
///
/// ## Creating and extending a dynamic array
///
/// ```
/// use voker_reflect::ops::{DynamicArray, Array};
///
/// let mut dynamic = DynamicArray::new();
/// dynamic.extend(1);
/// dynamic.extend(2);
/// dynamic.extend(3);
///
/// assert_eq!(dynamic.len(), 3);
/// ```
///
/// ## Converting from an iterator
///
/// ```
/// use voker_reflect::ops::{Array, DynamicArray};
///
/// let dynamic: DynamicArray = vec![1, 2, 3, 4, 5]
///     .into_iter()
///     .collect();
///
/// assert_eq!(dynamic.len(), 5);
/// ```
///
/// ## Applying to a static array
///
/// ```
/// use voker_reflect::{Reflect, ops::{Array, DynamicArray}};
///
/// let mut dynamic = DynamicArray::new();
/// dynamic.extend(10);
/// dynamic.extend(20);
/// dynamic.extend(30);
///
/// let mut arr = [0, 0, 0];
/// arr.apply(&dynamic);
///
/// assert_eq!(arr, [10, 20, 30]);
/// ```
///
/// [`reflect_kind`]: Reflect::reflect_kind
/// [`reflect_ref`]: Reflect::reflect_ref
/// [`extend`]: DynamicArray::extend
/// [`extend_boxed`]: DynamicArray::extend_boxed
/// [`represented_type_info`]: Reflect::represented_type_info
#[derive(Default)]
pub struct DynamicArray {
    info: Option<&'static TypeInfo>, // Ensure it is None or ArrayInfo
    values: Vec<Box<dyn Reflect>>,
}

// Explicitly implemented here so that code readers do not need
// to ponder the principles of proc-macros in advance.
impl TypePath for DynamicArray {
    #[inline]
    fn type_path() -> &'static str {
        "voker_reflect::ops::DynamicArray"
    }

    #[inline]
    fn type_name() -> &'static str {
        "DynamicArray"
    }

    #[inline]
    fn type_ident() -> &'static str {
        "DynamicArray"
    }

    #[inline]
    fn module_path() -> Option<&'static str> {
        Some("voker_reflect::ops")
    }
}

impl Typed for DynamicArray {
    fn type_info() -> &'static TypeInfo {
        static CELL: NonGenericTypeInfoCell = NonGenericTypeInfoCell::new();
        CELL.get_or_init(|| TypeInfo::Opaque(OpaqueInfo::new::<Self>()))
    }
}

impl DynamicArray {
    /// Creates an empty `DynamicArray`.
    ///
    /// If you already have data to populate the array, consider using
    /// [`FromIterator::from_iter`] instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::ops::{Array, DynamicArray};
    ///
    /// let dynamic = DynamicArray::new();
    /// assert_eq!(dynamic.len(), 0);
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self {
            info: None,
            values: Vec::new(),
        }
    }

    /// Creates a new empty `DynamicArray` with at least the specified capacity.
    ///
    /// This can be used to avoid reallocations when you know approximately
    /// how many elements will be added to the array.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            info: None,
            values: Vec::with_capacity(capacity),
        }
    }

    /// Sets the [`TypeInfo`] that this dynamic array represents.
    ///
    /// When set, [`Reflect::represented_type_info`] will return this information,
    /// allowing the dynamic array to be treated as if it were a specific static array type.
    ///
    /// # Panics
    ///
    /// Panics if `info` is `Some` but does not contain array type information.
    #[inline]
    pub const fn set_type_info(&mut self, info: Option<&'static TypeInfo>) {
        match info {
            Some(info) => {
                assert!(info.is_array(), "`TypeInfo` mismatched.");
                self.info = Some(info);
            }
            None => {
                self.info = None;
            }
        }
    }

    /// Appends a boxed [`Reflect`] value to the end of the array.
    ///
    /// This is the low-level version of [`extend`] that accepts already-boxed values.
    ///
    /// # Note
    ///
    /// While arrays in Rust have fixed lengths, `DynamicArray` can be extended
    /// dynamically. This makes it useful for building up arrays before applying
    /// them to static arrays of known size.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::ops::{Array, DynamicArray};
    /// let mut dynamic = DynamicArray::new();
    /// dynamic.extend_boxed(Box::new(1_i32));
    /// dynamic.extend_boxed(Box::new(2_i32));
    ///
    /// assert_eq!(dynamic.len(), 2);
    /// ```
    ///
    /// [`extend`]: DynamicArray::extend
    pub fn extend_boxed(&mut self, value: Box<dyn Reflect>) {
        self.values.push(value);
    }

    /// Appends a value to the end of the array.
    ///
    /// This is a convenience method that boxes the value and calls
    /// [`extend_boxed`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::ops::{Array, DynamicArray};
    /// let mut dynamic = DynamicArray::new();
    /// dynamic.extend(1_i32);
    /// dynamic.extend(2_i32);
    ///
    /// assert_eq!(dynamic.len(), 2);
    /// ```
    ///
    /// [`extend_boxed`]: DynamicArray::extend_boxed
    #[inline]
    pub fn extend<T: Reflect>(&mut self, value: T) {
        self.extend_boxed(Box::new(value));
    }
}

impl<T: Reflect> FromIterator<T> for DynamicArray {
    /// Creates a `DynamicArray` from an iterator of `Reflect` values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::ops::{Array, DynamicArray};
    /// let dynamic: DynamicArray = (0..5).collect();
    /// assert_eq!(dynamic.len(), 5);
    /// ```
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(values: I) -> Self {
        Self {
            info: None,
            values: values
                .into_iter()
                .map(Reflect::into_boxed_reflect)
                .collect(),
        }
    }
}

impl FromIterator<Box<dyn Reflect>> for DynamicArray {
    /// Creates a `DynamicArray` from an iterator of boxed `Reflect` values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::{Array, DynamicArray}};
    /// let values = vec![
    ///     Box::new(1_i32) as Box<dyn Reflect>,
    ///     Box::new(2_i32),
    ///     Box::new(3_i32),
    /// ];
    ///
    /// let dynamic: DynamicArray = values.into_iter().collect();
    /// assert_eq!(dynamic.len(), 3);
    /// ```
    #[inline]
    fn from_iter<I: IntoIterator<Item = Box<dyn Reflect>>>(values: I) -> Self {
        Self {
            info: None,
            values: values.into_iter().collect(),
        }
    }
}

impl Reflect for DynamicArray {
    crate::reflection::impl_reflect_cast_fn!(Array);

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
        Box::new(<Self as Array>::to_dynamic_array(self))
    }

    #[inline]
    fn reflect_clone(&self) -> Result<Box<dyn Reflect>, ReflectCloneError> {
        Ok(Box::new(<Self as Array>::to_dynamic_array(self)))
    }

    #[inline]
    fn apply(&mut self, value: &dyn Reflect) -> Result<(), ApplyError> {
        crate::impls::array_apply(self, value)
    }

    #[inline]
    fn reflect_hash(&self) -> Option<u64> {
        crate::impls::array_hash(self)
    }

    #[inline]
    fn reflect_eq(&self, other: &dyn Reflect) -> Option<bool> {
        crate::impls::array_eq(self, other)
    }

    #[inline]
    fn reflect_cmp(&self, other: &dyn Reflect) -> Option<Ordering> {
        crate::impls::array_cmp(self, other)
    }

    #[inline]
    fn reflect_debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DynamicArray(")?;
        crate::impls::array_debug(self, f)?;
        write!(f, ")")
    }
}

impl fmt::Debug for DynamicArray {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.reflect_debug(f)
    }
}

impl IntoIterator for DynamicArray {
    type Item = Box<dyn Reflect>;
    type IntoIter = alloc::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.values.into_iter()
    }
}

impl<'a> IntoIterator for &'a DynamicArray {
    type Item = &'a dyn Reflect;
    type IntoIter = ArrayItemIter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

// -----------------------------------------------------------------------------
// Array trait

/// A trait for type-erased array-like operations via reflection.
///
/// This trait represents any fixed-size linear sequence, including:
/// - Rust arrays (`[T; N]`)
/// - Types that can be viewed as arrays through reflection
///
/// This trait is automatically implemented for primitive array `[T; N]`
/// as long as `T` implemented [`Reflect`].
///
/// # Contract
///
/// Implementors must maintain a fixed size as returned by [`Array::len`].
/// While the trait allows accessing elements through reflection (which is
/// type-erased), implementors typically contain homogeneous elements
/// (all the same type) in practice.
///
/// # Examples
///
/// ## Using with static arrays
///
/// ```
/// use voker_reflect::ops::Array;
///
/// let arr = [10_u32, 20_u32, 30_u32];
/// let array_ref: &dyn Array = &arr;
///
/// assert_eq!(array_ref.len(), 3);
/// assert_eq!(array_ref.get_as::<u32>(1), Some(&20_u32));
/// ```
///
/// ## Iterating over elements
///
/// ```
/// # use voker_reflect::{Reflect, ops::Array};
/// let arr = ["foo", "bar", "baz"];
/// let array_ref: &dyn Array = &arr;
///
/// let elements: Vec<&str> = array_ref.iter()
///     .filter_map(<dyn Reflect>::downcast_ref::<&str>)
///     .copied()
///     .collect();
///
/// assert_eq!(elements, vec!["foo", "bar", "baz"]);
/// ```
///
/// [`len`]: Array::len
/// [`get`]: Array::get
/// [`get_mut`]: Array::get_mut
#[expect(clippy::len_without_is_empty, reason = "`len` is fixed for array.")]
pub trait Array: Reflect {
    /// Returns a reference to the element at the given index, or `None` if out of bounds.
    ///
    /// For type-safe access when the element type is known, use `<dyn Array>::get_as` instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::ops::Array;
    /// let arr = [1, 2, 3];
    /// let array_ref: &dyn Array = &arr;
    ///
    /// assert!(array_ref.get(0).is_some());
    /// assert!(array_ref.get(3).is_none());
    /// ```
    fn get(&self, index: usize) -> Option<&dyn Reflect>;

    /// Returns a mutable reference to the element at the given index, or `None` if out of bounds.
    ///
    /// For type-safe mutable access when the element type is known, use `<dyn Array>::get_mut_as` instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::{Reflect, ops::Array};
    ///
    /// let mut arr = [1, 2, 3];
    /// let array_ref: &mut dyn Array = &mut arr;
    ///
    /// if let Some(element) = array_ref.get_mut(1) {
    ///     *element.downcast_mut::<i32>().unwrap() = 99;
    /// }
    ///
    /// assert_eq!(arr, [1, 99, 3]);
    /// ```
    fn get_mut(&mut self, index: usize) -> Option<&mut dyn Reflect>;

    /// Returns the number of elements in the array.
    ///
    /// # Contract
    ///
    /// This value must remain constant for the lifetime of the object.
    /// Dynamic arrays that can change size should implement this trait carefully.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::ops::Array;
    /// let arr = [1, 2, 3, 4, 5];
    /// let array_ref: &dyn Array = &arr;
    ///
    /// assert_eq!(array_ref.len(), 5);
    /// ```
    fn len(&self) -> usize;

    /// Returns an iterator over the array's elements.
    ///
    /// The iterator yields references to each element in order,
    /// from index 0 to `len() - 1`.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::{Reflect, ops::Array};
    ///
    /// let arr = [10, 20, 30];
    /// let array_ref: &dyn Array = &arr;
    ///
    /// let sum: i32 = array_ref.iter()
    ///     .filter_map(<dyn Reflect>::downcast_ref::<i32>)
    ///     .sum();
    ///
    /// assert_eq!(sum, 60);
    /// ```
    fn iter(&self) -> ArrayItemIter<'_>;

    /// Consumes the boxed array and returns its elements as a vector.
    ///
    /// This is useful when you need to take ownership of the array's contents.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::ops::{Array, DynamicArray};
    ///
    /// let boxed: Box<dyn Array> = Box::new(DynamicArray::from_iter([1, 2, 3]));
    /// let elements = boxed.drain();
    ///
    /// assert_eq!(elements.len(), 3);
    /// ```
    fn drain(self: Box<Self>) -> Vec<Box<dyn Reflect>>;

    /// Creates a [`DynamicArray`] copy of this array.
    ///
    /// This is useful when you need a mutable, resizable version of a static array.
    ///
    /// This function will replace all content with dynamic types, except for opaque types.
    ///
    /// # Panics
    ///
    /// Panic if inner items [`Reflect::to_dynamic`] failed.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::ops::{DynamicArray, Array};
    /// let arr = [1, 2, 3];
    /// let dynamic: DynamicArray = arr.to_dynamic_array();
    /// ```
    fn to_dynamic_array(&self) -> DynamicArray {
        DynamicArray {
            info: self.represented_type_info(),
            values: self.iter().map(Reflect::to_dynamic).collect(),
        }
    }
}

impl Array for DynamicArray {
    #[inline]
    fn get(&self, index: usize) -> Option<&dyn Reflect> {
        self.values.get(index).map(core::ops::Deref::deref)
    }

    #[inline]
    fn get_mut(&mut self, index: usize) -> Option<&mut dyn Reflect> {
        self.values
            .get_mut(index)
            .map(core::ops::DerefMut::deref_mut)
    }

    #[inline]
    fn len(&self) -> usize {
        self.values.len()
    }

    #[inline]
    fn iter(&self) -> ArrayItemIter<'_> {
        ArrayItemIter::new(self)
    }

    #[inline]
    fn drain(self: Box<Self>) -> Vec<Box<dyn Reflect>> {
        self.values
    }
}

impl dyn Array {
    /// Returns a typed reference to the element at the given index.
    ///
    /// Returns `None` if:
    /// - The index is out of bounds
    /// - The element cannot be downcast to type `T`
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::ops::Array;
    /// let arr = [10_i32, 20_i32, 30_i32];
    /// let array_ref: &dyn Array = &arr;
    ///
    /// assert_eq!(array_ref.get_as::<i32>(1), Some(&20));
    /// assert_eq!(array_ref.get_as::<i32>(5), None); // Out of bounds
    /// ```
    #[inline]
    pub fn get_as<T: Reflect>(&self, index: usize) -> Option<&T> {
        self.get(index).and_then(<dyn Reflect>::downcast_ref)
    }

    /// Returns a typed mutable reference to the element at the given index.
    ///
    /// Returns `None` if:
    /// - The index is out of bounds
    /// - The element cannot be downcast to type `T`
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::ops::Array;
    /// let mut arr = [10_i32, 20_i32, 30_i32];
    /// let array_ref: &mut dyn Array = &mut arr;
    ///
    /// if let Some(element) = array_ref.get_mut_as::<i32>(1) {
    ///     *element = 99;
    /// }
    ///
    /// assert_eq!(arr, [10, 99, 30]);
    /// ```
    #[inline]
    pub fn get_mut_as<T: Reflect>(&mut self, index: usize) -> Option<&mut T> {
        self.get_mut(index).and_then(<dyn Reflect>::downcast_mut)
    }
}

// -----------------------------------------------------------------------------
// Array Iterator

/// An iterator over the elements of an [`Array`].
///
/// This is an [`ExactSizeIterator`] that yields references to each element
/// in the array in order.
///
/// # Performance
///
/// The iterator uses [`Array::get`] internally, which may have different
/// performance characteristics than iterating directly over a concrete array type.
///
/// # Examples
///
/// ```
/// # use voker_reflect::{Reflect, ops::{Array, ArrayItemIter}};
/// let arr = [1, 2, 3, 4, 5];
/// let mut iter = ArrayItemIter::new(&arr);
///
/// assert_eq!(iter.len(), 5);
/// assert_eq!(iter.next().and_then(|v| v.downcast_ref::<i32>()), Some(&1));
/// ```
pub struct ArrayItemIter<'a> {
    array: &'a dyn Array,
    index: usize,
}

impl ArrayItemIter<'_> {
    /// Create a [`ArrayItemIter`] from a [`Array`].
    #[inline(always)]
    pub const fn new(array: &dyn Array) -> ArrayItemIter<'_> {
        ArrayItemIter { array, index: 0 }
    }
}

impl<'a> Iterator for ArrayItemIter<'a> {
    type Item = &'a dyn Reflect;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let value = self.array.get(self.index);
        self.index += value.is_some() as usize;
        value
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let hint = self.array.len() - self.index;
        (hint, Some(hint))
    }
}

impl ExactSizeIterator for ArrayItemIter<'_> {}
impl FusedIterator for ArrayItemIter<'_> {}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use super::DynamicArray;
    use crate::info::TypePath;

    #[test]
    fn type_path() {
        assert!(DynamicArray::type_path() == "voker_reflect::ops::DynamicArray");
        assert!(DynamicArray::module_path() == Some("voker_reflect::ops"));
        assert!(DynamicArray::type_ident() == "DynamicArray");
        assert!(DynamicArray::type_name() == "DynamicArray");
    }
}
