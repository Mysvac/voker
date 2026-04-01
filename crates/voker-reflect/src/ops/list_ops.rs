use alloc::{boxed::Box, vec::Vec};
use core::cmp::Ordering;
use core::fmt;
use core::iter::FusedIterator;

use crate::Reflect;
use crate::impls::NonGenericTypeInfoCell;
use crate::info::{OpaqueInfo, TypeInfo, TypePath, Typed};
use crate::ops::{ApplyError, ReflectCloneError};

// -----------------------------------------------------------------------------
// Dynamic List

/// A dynamic container representing a list-like collection.
///
/// `DynamicList` is a type-erased dynamic list that can hold any type implementing
/// [`Reflect`]. It represents variable-length collections like [`Vec`] in Rust.
///
/// Unlike arrays, lists are expected to have variable lengths, so operations like
/// [`extend`] and [`push`] are natural extensions of the list's functionality.
///
/// # Type Information
///
/// Dynamic types are special in that their `TypeInfo` is [`OpaqueInfo`],
/// but other APIs behave like the represented type, such as [`reflect_kind`] and [`reflect_ref`].
///
/// A `DynamicList` can optionally represent a specific list type through its
/// [`represented_type_info`]. When set, this allows the dynamic list to be treated
/// as if it were a specific static list type for reflection purposes.
///
/// # Examples
///
/// ## Creating and extending a dynamic list
///
/// ```
/// use voker_reflect::ops::{List, DynamicList};
///
/// let mut dynamic = DynamicList::new();
/// dynamic.extend(1);
/// dynamic.extend(2);
/// dynamic.extend(3);
///
/// assert_eq!(dynamic.len(), 3);
/// ```
///
/// ## Using list operations
///
/// ```
/// use voker_reflect::{Reflect, ops::{DynamicList, List}};
///
/// let mut dynamic = DynamicList::new();
/// dynamic.push(Box::new(1_i32));
/// dynamic.push(Box::new(2_i32));
/// dynamic.insert(1, Box::new(99_i32));
///
/// assert_eq!(dynamic.len(), 3);
/// ```
///
/// [`reflect_kind`]: Reflect::reflect_kind
/// [`reflect_ref`]: Reflect::reflect_ref
/// [`represented_type_info`]: Reflect::represented_type_info
/// [`extend`]: DynamicList::extend
/// [`push`]: List::push
/// [`DynamicArray`]: crate::ops::DynamicArray
#[derive(Default)]
pub struct DynamicList {
    info: Option<&'static TypeInfo>,
    values: Vec<Box<dyn Reflect>>,
}

// Explicitly implemented here so that code readers do not need
// to ponder the principles of proc-macros in advance.
impl TypePath for DynamicList {
    #[inline]
    fn type_path() -> &'static str {
        "voker_reflect::ops::DynamicList"
    }

    #[inline]
    fn type_name() -> &'static str {
        "DynamicList"
    }

    #[inline]
    fn type_ident() -> &'static str {
        "DynamicList"
    }

    #[inline]
    fn module_path() -> Option<&'static str> {
        Some("voker_reflect::ops")
    }
}

impl Typed for DynamicList {
    fn type_info() -> &'static TypeInfo {
        static CELL: NonGenericTypeInfoCell = NonGenericTypeInfoCell::new();
        CELL.get_or_init(|| TypeInfo::Opaque(OpaqueInfo::new::<Self>()))
    }
}

impl DynamicList {
    /// Creates an empty `DynamicList`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::ops::{List, DynamicList};
    /// let dynamic = DynamicList::new();
    /// assert!(dynamic.is_empty());
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self {
            info: None,
            values: Vec::new(),
        }
    }

    /// Creates a new empty `DynamicList` with at least the specified capacity.
    ///
    /// This can be used to avoid reallocations when you know approximately
    /// how many elements will be added to the list.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            info: None,
            values: Vec::with_capacity(capacity),
        }
    }

    /// Sets the [`TypeInfo`] that this dynamic list represents.
    ///
    /// When set, [`Reflect::represented_type_info`] will return this information,
    /// allowing the dynamic list to be treated as if it were a specific static list type.
    ///
    /// # Panics
    ///
    /// Panics if `info` is `Some` but does not contain list type information.
    pub const fn set_type_info(&mut self, info: Option<&'static TypeInfo>) {
        match info {
            Some(info) => {
                assert!(info.is_list(), "`TypeInfo` mismatched.");
                self.info = Some(info);
            }
            None => {
                self.info = None;
            }
        }
    }

    /// Appends a boxed [`Reflect`] value to the end of the list.
    ///
    /// This is the low-level version of [`extend`] that accepts already-boxed values.
    ///
    /// # Note
    ///
    /// This method is equivalent to [`List::push`] but provided for consistency
    /// with `DynamicArray`'s API.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::ops::{List, DynamicList};
    ///
    /// let mut dynamic = DynamicList::new();
    /// dynamic.extend_boxed(Box::new(1_i32));
    /// dynamic.extend_boxed(Box::new(2_i32));
    ///
    /// assert_eq!(dynamic.len(), 2);
    /// ```
    ///
    /// [`extend`]: DynamicList::extend
    pub fn extend_boxed(&mut self, value: Box<dyn Reflect>) {
        self.values.push(value);
    }

    /// Appends a value to the end of the list.
    ///
    /// This is a convenience method that boxes the value and calls
    /// [`extend_boxed`].
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::ops::{List, DynamicList};
    ///
    /// let mut dynamic = DynamicList::new();
    /// dynamic.extend(1_i32);
    /// dynamic.extend(2_i32);
    /// dynamic.extend(3_i32);
    ///
    /// assert_eq!(dynamic.len(), 3);
    /// ```
    ///
    /// [`extend_boxed`]: DynamicList::extend_boxed
    #[inline]
    pub fn extend<T: Reflect>(&mut self, value: T) {
        self.extend_boxed(Box::new(value));
    }
}

impl<T: Reflect> FromIterator<T> for DynamicList {
    fn from_iter<I: IntoIterator<Item = T>>(values: I) -> Self {
        Self {
            info: None,
            values: values.into_iter().map(Reflect::into_boxed_reflect).collect(),
        }
    }
}

impl FromIterator<Box<dyn Reflect>> for DynamicList {
    fn from_iter<I: IntoIterator<Item = Box<dyn Reflect>>>(values: I) -> Self {
        Self {
            info: None,
            values: values.into_iter().collect(),
        }
    }
}

impl Reflect for DynamicList {
    crate::reflection::impl_reflect_cast_fn!(List);

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
        Box::new(<Self as List>::to_dynamic_list(self))
    }

    #[inline]
    fn reflect_clone(&self) -> Result<Box<dyn Reflect>, ReflectCloneError> {
        Ok(Box::new(<Self as List>::to_dynamic_list(self)))
    }

    #[inline]
    fn apply(&mut self, value: &dyn Reflect) -> Result<(), ApplyError> {
        crate::impls::list_apply(self, value)
    }

    #[inline]
    fn reflect_hash(&self) -> Option<u64> {
        crate::impls::list_hash(self)
    }

    #[inline]
    fn reflect_eq(&self, other: &dyn Reflect) -> Option<bool> {
        crate::impls::list_eq(self, other)
    }

    #[inline]
    fn reflect_cmp(&self, other: &dyn Reflect) -> Option<Ordering> {
        crate::impls::list_cmp(self, other)
    }

    #[inline]
    fn reflect_debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DynamicList(")?;
        crate::impls::list_debug(self, f)?;
        write!(f, ")")
    }
}

impl fmt::Debug for DynamicList {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.reflect_debug(f)
    }
}

impl IntoIterator for DynamicList {
    type Item = Box<dyn Reflect>;
    type IntoIter = alloc::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.values.into_iter()
    }
}

impl<'a> IntoIterator for &'a DynamicList {
    type Item = &'a dyn Reflect;
    type IntoIter = ListItemIter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

// -----------------------------------------------------------------------------
// List trait

/// A trait for type-erased list-like operations via reflection.
///
/// This trait represents any variable-length sequential collection, including:
/// - Rust vectors (`Vec<T>`)
/// - Linked lists (`LinkedList<T>`)
/// - Other collections that support sequential access and modification
///
/// We implemented this trait for some common type, such as [`Vec`], [`VecDeque`](alloc::collections::VecDeque).
///
/// # Contract
///
/// Implementors must maintain elements in linear order from front to back,
/// where the front element is at index 0 and the back element is at the largest index.
/// Lists can grow and shrink dynamically through methods like [`push`], [`pop`],
/// [`insert`], and [`remove`].
///
/// Unlike the [`Array`](crate::ops::Array) trait, lists are expected to support
/// dynamic resizing as part of their normal operation.
///
/// # Safety
///
/// This trait is safe to implement. However, implementors must ensure that:
/// 1. Elements are stored in sequential order
/// 2. Index-based operations maintain proper bounds checking
/// 3. Mutating operations preserve the integrity of the list
///
/// # Examples
///
/// ## Using with vectors
///
/// ```
/// use voker_reflect::{Reflect, ops::List};
///
/// let mut vec = vec![10_u32, 20_u32, 30_u32];
/// let list_ref: &mut dyn List = &mut vec;
///
/// assert_eq!(list_ref.len(), 3);
/// list_ref.push(Box::new(40_u32));
/// assert_eq!(list_ref.len(), 4);
/// ```
///
/// ## Modifying list contents
///
/// ```
/// use voker_reflect::{Reflect, ops::List};
///
/// let mut vec = vec!["first", "second", "third"];
/// let list_ref: &mut dyn List = &mut vec;
///
/// list_ref.insert(1, Box::new("inserted"));
/// let removed = list_ref.remove(2);
///
/// assert_eq!(list_ref.len(), 3);
/// assert_eq!(removed.downcast_ref::<&str>(), Some(&"second"));
/// ```
///
/// [`push`]: List::push
/// [`pop`]: List::pop
/// [`insert`]: List::insert
/// [`remove`]: List::remove
pub trait List: Reflect {
    /// Returns a reference to the element at the given index, or `None` if out of bounds.
    ///
    /// For type-safe access when the element type is known, use `<dyn List>::get_as` instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::ops::List;
    /// let vec = vec![1, 2, 3];
    /// let list_ref: &dyn List = &vec;
    ///
    /// assert!(list_ref.get(0).is_some());
    /// assert!(list_ref.get(3).is_none());
    /// ```
    fn get(&self, index: usize) -> Option<&dyn Reflect>;

    /// Returns a mutable reference to the element at the given index, or `None` if out of bounds.
    ///
    /// For type-safe mutable access when the element type is known, use `<dyn List>::get_mut_as` instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::{Reflect, ops::List};
    ///
    /// let mut vec = vec![1, 2, 3];
    /// let list_ref: &mut dyn List = &mut vec;
    ///
    /// if let Some(element) = list_ref.get_mut(1) {
    ///     *element.downcast_mut::<i32>().unwrap() = 99;
    /// }
    ///
    /// assert_eq!(vec, vec![1, 99, 3]);
    /// ```
    fn get_mut(&mut self, index: usize) -> Option<&mut dyn Reflect>;

    /// Inserts an element at the specified position in the list.
    ///
    /// All elements after `index` are shifted to the right (their indices increase by 1).
    ///
    /// In standard implementation (e.g. `Vec<T>`), this function will use
    /// [`FromReflect::take_from_reflect`] to convert value.
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - `index > len`
    /// - The element type is incompatible with the list (implementation-specific)
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::ops::List;
    /// let mut vec = vec![1, 3];
    /// let list_ref: &mut dyn List = &mut vec;
    ///
    /// list_ref.insert(1, Box::new(2));
    /// assert_eq!(list_ref.len(), 3);
    /// ```
    ///
    /// [`FromReflect::take_from_reflect`]: crate::FromReflect::take_from_reflect
    fn insert(&mut self, index: usize, element: Box<dyn Reflect>);

    /// Removes and returns the element at the specified position in the list.
    ///
    /// All elements after `index` are shifted to the left (their indices decrease by 1).
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds (`index >= len`).
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::{Reflect, ops::List};
    ///
    /// let mut vec = vec![1, 2, 3];
    /// let list_ref: &mut dyn List = &mut vec;
    ///
    /// let removed = list_ref.remove(1);
    /// assert_eq!(removed.downcast_ref::<i32>(), Some(&2));
    /// assert_eq!(list_ref.len(), 2);
    /// ```
    fn remove(&mut self, index: usize) -> Box<dyn Reflect>;

    /// Appends an element to the end of the list.
    ///
    /// In standard implementation (e.g. `Vec<T>`), this function will use
    /// [`FromReflect::take_from_reflect`] to convert value.
    ///
    /// # Panics
    ///
    /// Panics if the element type is incompatible with the list (implementation-specific).
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::{Reflect, ops::List};
    ///
    /// let mut vec = vec![1, 2];
    /// let list_ref: &mut dyn List = &mut vec;
    ///
    /// list_ref.push(Box::new(3));
    /// assert_eq!(list_ref.len(), 3);
    /// ```
    ///
    /// [`FromReflect::take_from_reflect`]: crate::FromReflect::take_from_reflect
    fn push(&mut self, value: Box<dyn Reflect>);

    /// Attempts to append an element to the end of the list.
    ///
    /// This is a non-panicking version of [`push`].
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the element was successfully appended
    /// * `Err(value)` if the element type is incompatible with the list
    ///   (the element is returned unchanged)
    ///
    /// # Note
    ///
    /// The returned error value must be the same as the input value.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::ops::List;
    /// let mut vec = vec![1, 2];
    /// let list_ref: &mut dyn List = &mut vec;
    ///
    /// assert!(list_ref.try_push(Box::new(3)).is_ok());
    ///
    /// // If type checking fails, the value is returned
    /// let element = Box::new("string");
    /// let result = list_ref.try_push(element);
    /// assert!(result.is_err());
    /// ```
    ///
    /// [`push`]: List::push
    fn try_push(&mut self, value: Box<dyn Reflect>) -> Result<(), Box<dyn Reflect>>;

    /// Removes and returns the last element of the list, or `None` if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::List};
    /// let mut vec = vec![1, 2, 3];
    /// let list_ref: &mut dyn List = &mut vec;
    ///
    /// let last = list_ref.pop();
    /// assert_eq!(last.unwrap().downcast_ref::<i32>(), Some(&3));
    /// assert_eq!(list_ref.len(), 2);
    ///
    /// // Empty list
    /// let mut empty: Vec<i32> = vec![];
    /// let empty_ref: &mut dyn List = &mut empty;
    /// assert!(empty_ref.pop().is_none());
    /// ```
    fn pop(&mut self) -> Option<Box<dyn Reflect>>;

    /// Returns the number of elements in the list.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::List};
    /// let vec = vec![1, 2, 3, 4, 5];
    /// let list_ref: &dyn List = &vec;
    ///
    /// assert_eq!(list_ref.len(), 5);
    /// ```
    fn len(&self) -> usize;

    /// Returns `true` if the list contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::List};
    /// let empty: Vec<i32> = vec![];
    /// let non_empty = vec![1, 2, 3];
    ///
    /// let empty_ref: &dyn List = &empty;
    /// let non_empty_ref: &dyn List = &non_empty;
    ///
    /// assert!(empty_ref.is_empty());
    /// assert!(!non_empty_ref.is_empty());
    /// ```
    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns an iterator over the list's elements.
    ///
    /// The iterator yields references to each element in order,
    /// from index 0 to `len() - 1`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::List};
    /// let vec = vec![10, 20, 30];
    /// let list_ref: &dyn List = &vec;
    ///
    /// let sum: i32 = list_ref.iter()
    ///     .filter_map(|v| v.downcast_ref::<i32>())
    ///     .sum();
    ///
    /// assert_eq!(sum, 60);
    /// ```
    fn iter(&self) -> ListItemIter<'_>;

    /// Removes all elements from the list and returns them as a vector.
    ///
    /// After calling this method, the list will be empty. The elements are returned
    /// in the same order they appeared in the list.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::List};
    /// let mut vec = vec![1, 2, 3];
    /// let list_ref: &mut dyn List = &mut vec;
    ///
    /// let drained = list_ref.drain();
    /// assert!(list_ref.is_empty());
    /// assert_eq!(drained.len(), 3);
    /// ```
    fn drain(&mut self) -> Vec<Box<dyn Reflect>>;

    /// Creates a [`DynamicList`] copy of this list.
    ///
    /// This is useful when you need a dynamic, mutable copy of a list.
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
    /// # use voker_reflect::{Reflect, ops::List};
    /// let vec = vec![1, 2, 3];
    /// let dynamic = vec.to_dynamic_list();
    ///
    /// assert_eq!(dynamic.len(), 3);
    /// ```
    fn to_dynamic_list(&self) -> DynamicList {
        DynamicList {
            info: self.represented_type_info(),
            values: self.iter().map(Reflect::to_dynamic).collect(),
        }
    }
}

impl List for DynamicList {
    #[inline]
    fn get(&self, index: usize) -> Option<&dyn Reflect> {
        self.values.get(index).map(core::ops::Deref::deref)
    }

    #[inline]
    fn get_mut(&mut self, index: usize) -> Option<&mut dyn Reflect> {
        self.values.get_mut(index).map(core::ops::DerefMut::deref_mut)
    }

    #[inline]
    fn insert(&mut self, index: usize, element: Box<dyn Reflect>) {
        self.values.insert(index, element);
    }

    #[inline]
    fn remove(&mut self, index: usize) -> Box<dyn Reflect> {
        self.values.remove(index)
    }

    #[inline]
    fn push(&mut self, value: Box<dyn Reflect>) {
        self.values.push(value);
    }

    #[inline]
    fn try_push(&mut self, value: Box<dyn Reflect>) -> Result<(), Box<dyn Reflect>> {
        self.values.push(value);
        Ok(())
    }

    #[inline]
    fn pop(&mut self) -> Option<Box<dyn Reflect>> {
        self.values.pop()
    }

    #[inline]
    fn len(&self) -> usize {
        self.values.len()
    }

    #[inline]
    fn iter(&self) -> ListItemIter<'_> {
        ListItemIter::new(self)
    }

    #[inline]
    fn drain(&mut self) -> Vec<Box<dyn Reflect>> {
        self.values.drain(..).collect()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl dyn List {
    /// Returns a typed reference to the element at the given index.
    ///
    /// Returns `None` if:
    /// - The index is out of bounds
    /// - The element cannot be downcast to type `T`
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::ops::List;
    /// let vec = vec![10_i32, 20_i32, 30_i32];
    /// let list_ref: &dyn List = &vec;
    ///
    /// assert_eq!(list_ref.get_as::<i32>(1), Some(&20));
    /// assert_eq!(list_ref.get_as::<i32>(5), None); // Out of bounds
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
    /// # use voker_reflect::{Reflect, ops::List};
    /// let mut vec = vec![10_i32, 20_i32, 30_i32];
    /// let list_ref: &mut dyn List = &mut vec;
    ///
    /// if let Some(element) = list_ref.get_mut_as::<i32>(1) {
    ///     *element = 99;
    /// }
    ///
    /// assert_eq!(vec, vec![10, 99, 30]);
    /// ```
    #[inline]
    pub fn get_mut_as<T: Reflect>(&mut self, index: usize) -> Option<&mut T> {
        self.get_mut(index).and_then(<dyn Reflect>::downcast_mut)
    }
}

// -----------------------------------------------------------------------------
// List Iterator

/// An iterator over the elements of a [`List`].
///
/// This is an [`ExactSizeIterator`] that yields references to each element
/// in the list in order.
///
/// # Performance
///
/// The iterator uses [`List::get`] internally, which may have different
/// performance characteristics than iterating directly over a concrete list type.
///
/// # Examples
///
/// ```
/// use voker_reflect::{Reflect, ops::{List, ListItemIter}};
///
/// let vec = vec![1, 2, 3, 4, 5];
/// let mut iter = ListItemIter::new(&vec);
///
/// assert_eq!(iter.len(), 5);
/// assert_eq!(iter.next().and_then(|v| v.downcast_ref::<i32>()), Some(&1));
/// ```
pub struct ListItemIter<'a> {
    list: &'a dyn List,
    index: usize,
}

impl ListItemIter<'_> {
    /// Creates a new iterator for the given list.
    #[inline(always)]
    pub const fn new(list: &dyn List) -> ListItemIter<'_> {
        ListItemIter { list, index: 0 }
    }
}

impl<'a> Iterator for ListItemIter<'a> {
    type Item = &'a dyn Reflect;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let value = self.list.get(self.index);
        self.index += value.is_some() as usize;
        value
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let hint = self.list.len() - self.index;
        (hint, Some(hint))
    }
}

impl ExactSizeIterator for ListItemIter<'_> {}
impl FusedIterator for ListItemIter<'_> {}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use super::DynamicList;
    use crate::info::TypePath;

    #[test]
    fn type_path() {
        assert!(DynamicList::type_path() == "voker_reflect::ops::DynamicList");
        assert!(DynamicList::module_path() == Some("voker_reflect::ops"));
        assert!(DynamicList::type_ident() == "DynamicList");
        assert!(DynamicList::type_name() == "DynamicList");
    }
}
