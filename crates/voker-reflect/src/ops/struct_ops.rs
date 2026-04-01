use alloc::borrow::{Cow, ToOwned};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;
use core::iter::FusedIterator;
use core::ops::{Deref, DerefMut};

use voker_utils::hash::HashMap;

use crate::Reflect;
use crate::impls::NonGenericTypeInfoCell;
use crate::info::{OpaqueInfo, TypeInfo, TypePath, Typed};
use crate::ops::{ApplyError, ReflectCloneError};
use crate::reflection::impl_reflect_cast_fn;

// -----------------------------------------------------------------------------
// Dynamic Struct

/// A dynamic container representing a struct.
///
/// `DynamicStruct` is a type-erased dynamic struct that can hold any types
/// implementing [`Reflect`].
///
/// `DynamicStruct` can change its fields dynamically using [`extend`] or [`extend_boxed`].
///
/// # Type Information
///
/// Dynamic types are special in that their `TypeInfo` is [`OpaqueInfo`],
/// but other APIs behave like the represented type, such as [`reflect_kind`] and [`reflect_ref`].
///
/// A `DynamicStruct` can optionally represent a specific struct type through its
/// [`represented_type_info`]. When set, this allows the dynamic struct to be treated
/// as if it were a specific static struct type for reflection purposes.
///
/// But remember, we do not check whether the number and type of elements inside
/// the container are correct, and users need to pay attention to it.
///
/// # Examples
///
/// ## Creating and extending a dynamic struct
///
/// ```
/// use voker_reflect::ops::{DynamicStruct, Struct};
///
/// let mut dynamic = DynamicStruct::new();
/// dynamic.extend("field_1", 1_i32);
/// dynamic.extend("field_2", "hello");
/// dynamic.extend("field_3", true);
///
/// assert_eq!(dynamic.field_len(), 3);
/// ```
///
/// ## Applying to a static struct
///
/// ```
/// use voker_reflect::{Reflect, ops::{Struct, DynamicStruct}};
///
/// #[derive(Reflect, PartialEq, Debug)]
/// struct Foo{
///     field_a: i32,
///     field_b: bool,
/// };
///
/// let mut dynamic = DynamicStruct::new();
/// dynamic.extend("field_a", 10_i32);
/// dynamic.extend("field_b", true);
///
/// let mut foo = Foo{
///     field_a: 0,
///     field_b: false,
/// };
/// foo.apply(&dynamic);
///
/// assert_eq!(
///     foo,
///     Foo{
///         field_a: 10_i32,
///         field_b: true,
///     }
/// );
/// ```
///
/// [`reflect_kind`]: Reflect::reflect_kind
/// [`reflect_ref`]: Reflect::reflect_ref
/// [`extend`]: DynamicStruct::extend
/// [`extend_boxed`]: DynamicStruct::extend_boxed
/// [`represented_type_info`]: Reflect::represented_type_info
#[derive(Default)]
pub struct DynamicStruct {
    info: Option<&'static TypeInfo>,
    fields: Vec<Box<dyn Reflect>>,
    field_names: Vec<Cow<'static, str>>,
    field_indices: HashMap<Cow<'static, str>, usize>,
}

// Explicitly implemented here so that code readers do not need
// to ponder the principles of proc-macros in advance.
impl TypePath for DynamicStruct {
    #[inline]
    fn type_path() -> &'static str {
        "voker_reflect::ops::DynamicStruct"
    }

    #[inline]
    fn type_name() -> &'static str {
        "DynamicStruct"
    }

    #[inline]
    fn type_ident() -> &'static str {
        "DynamicStruct"
    }

    #[inline]
    fn module_path() -> Option<&'static str> {
        Some("voker_reflect::ops")
    }
}

impl Typed for DynamicStruct {
    fn type_info() -> &'static TypeInfo {
        static CELL: NonGenericTypeInfoCell = NonGenericTypeInfoCell::new();
        CELL.get_or_init(|| TypeInfo::Opaque(OpaqueInfo::new::<Self>()))
    }
}

impl DynamicStruct {
    /// Creates an empty `DynamicStruct`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::ops::DynamicStruct;
    /// let dynamic = DynamicStruct::new();
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self {
            info: None,
            fields: Vec::new(),
            field_names: Vec::new(),
            field_indices: HashMap::new(),
        }
    }

    /// Creates a new empty `DynamicStruct` with at least the specified capacity.
    ///
    /// This can be used to avoid reallocations when you know approximately
    /// how many fields will be added to the tuple.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            info: None,
            fields: Vec::with_capacity(capacity),
            field_names: Vec::with_capacity(capacity),
            field_indices: HashMap::with_capacity(capacity),
        }
    }

    /// Sets the [`TypeInfo`] that this dynamic struct represents.
    ///
    /// When set, [`Reflect::represented_type_info`] will return this information,
    /// allowing the dynamic struct to be treated as if it were a specific static struct type.
    ///
    /// # Panics
    ///
    /// Panics if `info` is `Some` but does not contain struct type information.
    #[inline]
    pub const fn set_type_info(&mut self, info: Option<&'static TypeInfo>) {
        match info {
            Some(info) => {
                assert!(info.is_struct(), "`TypeInfo` mismatched.");
                self.info = Some(info);
            }
            None => {
                self.info = None;
            }
        }
    }

    /// Appends a boxed [`Reflect`] value to the end of the struct as a field.
    ///
    /// If the field name already exists, this will overwrite it.
    ///
    /// This is the low-level version of [`extend`] that accepts already-boxed values.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::ops::{Struct, DynamicStruct};
    ///
    /// let mut dynamic = DynamicStruct::new();
    /// dynamic.extend_boxed("field_a", Box::new(1_i32));
    /// dynamic.extend_boxed("field_b", Box::new("hello"));
    ///
    /// assert_eq!(dynamic.field_len(), 2);
    /// ```
    ///
    /// [`extend`]: DynamicStruct::extend
    pub fn extend_boxed(&mut self, name: impl Into<Cow<'static, str>>, value: Box<dyn Reflect>) {
        let name: Cow<'static, str> = name.into();
        if let Some(index) = self.field_indices.get(&name) {
            self.fields[*index] = value;
        } else {
            self.fields.push(value);
            self.field_indices.insert(name.clone(), self.fields.len() - 1);
            self.field_names.push(name);
        }
    }

    /// Appends a value to the end of the struct as a field.
    ///
    /// If the field name already exists, this will overwrite it.
    ///
    /// This is a convenience method that boxes the value and calls
    /// [`extend_boxed`].
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::ops::{Struct, DynamicStruct};
    ///
    /// let mut dynamic = DynamicStruct::new();
    /// dynamic.extend("field_a", 42_i32);
    /// dynamic.extend("field_b", "world");
    ///
    /// assert_eq!(dynamic.field_len(), 2);
    /// ```
    ///
    /// [`extend_boxed`]: DynamicStruct::extend_boxed
    #[inline]
    pub fn extend<T: Reflect>(&mut self, name: impl Into<Cow<'static, str>>, value: T) {
        self.extend_boxed(name, Box::new(value));
    }
}

impl Reflect for DynamicStruct {
    impl_reflect_cast_fn!(Struct);

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
        Box::new(<Self as Struct>::to_dynamic_struct(self))
    }

    #[inline]
    fn reflect_clone(&self) -> Result<Box<dyn Reflect>, ReflectCloneError> {
        Ok(Box::new(<Self as Struct>::to_dynamic_struct(self)))
    }

    #[inline]
    fn apply(&mut self, value: &dyn Reflect) -> Result<(), ApplyError> {
        crate::impls::struct_apply(self, value)
    }

    #[inline]
    fn reflect_hash(&self) -> Option<u64> {
        crate::impls::struct_hash(self)
    }

    #[inline]
    fn reflect_eq(&self, other: &dyn Reflect) -> Option<bool> {
        crate::impls::struct_eq(self, other)
    }

    #[inline]
    fn reflect_cmp(&self, other: &dyn Reflect) -> Option<Ordering> {
        crate::impls::struct_cmp(self, other)
    }

    #[inline]
    fn reflect_debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DynamicStruct(")?;
        crate::impls::struct_debug(self, f)?;
        write!(f, ")")
    }
}

impl fmt::Debug for DynamicStruct {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.reflect_debug(f)
    }
}

impl<N: Into<Cow<'static, str>>> FromIterator<(N, Box<dyn Reflect>)> for DynamicStruct {
    fn from_iter<T: IntoIterator<Item = (N, Box<dyn Reflect>)>>(fields: T) -> Self {
        let mut dynamic_struct = DynamicStruct::new();
        for (name, value) in fields.into_iter() {
            dynamic_struct.extend_boxed(name, value);
        }
        dynamic_struct
    }
}

impl IntoIterator for DynamicStruct {
    type Item = Box<dyn Reflect>;
    type IntoIter = alloc::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.fields.into_iter()
    }
}

impl<'a> IntoIterator for &'a DynamicStruct {
    type Item = &'a dyn Reflect;
    type IntoIter = StructFieldIter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_fields()
    }
}

// -----------------------------------------------------------------------------
// Struct trait

/// A trait for type-erased struct operations via reflection.
///
/// This trait represents any fixed-size heterogeneous collection, including:
/// - Rust structs (e.g. `Foo{ id: i32, name: String }`)
/// - Types that can be viewed as structs through reflection
///
/// When using [`#[derive(Reflect)]`](crate::derive::Reflect) on a standard struct,
/// this trait will be automatically implemented.
///
/// # Note
///
/// This includes `struct T{}`, but not `struct T;`.
/// The latter will be considered as [`Opaque`](crate::info::OpaqueInfo) type
/// and can be optimized extensively.
///
/// # Examples
///
/// ```
/// use voker_reflect::{Reflect, ops::Struct};
///
/// #[derive(Reflect)]
/// struct Foo{
///     a: i32,
///     b: bool,
/// };
///
/// let ts = Foo{ a: 10_i32, b: true };
/// let ts_ref: &dyn Struct = &ts;
///
/// assert_eq!(ts_ref.field_len(), 2);
/// assert_eq!(ts_ref.field_as::<i32>("a"), Some(&10));
/// assert_eq!(ts_ref.field_at_as::<bool>(1), Some(&true));
/// ```
pub trait Struct: Reflect {
    /// Returns a reference to the value of the field named `name` as a
    /// `&dyn Reflect`.
    ///
    /// Returns `None` if the field does not exist.
    ///
    /// If the field type is known, can use `<dyn Struct>::field_as` instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::Struct};
    /// #[derive(Reflect)]
    /// struct Foo{ a: i32, b: bool };
    ///
    /// let ts = Foo{ a: 1, b: true };
    ///
    /// assert!(ts.field("a").is_some());
    /// assert!(ts.field("c").is_none());
    /// ```
    fn field(&self, name: &str) -> Option<&dyn Reflect>;

    /// Returns a mutable reference to the value of the field named `name`
    /// as a `&mut dyn Reflect`.
    ///
    /// Returns `None` if the field does not exist.
    ///
    /// If the field type is known, can use `<dyn Struct>::field_mut_as` instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::Struct};
    /// #[derive(Reflect)]
    /// struct Foo{ a: i32, b: bool };
    ///
    /// let mut ts = Foo{ a: 1, b: true };
    ///
    /// if let Some(field) = ts.field_mut("a") {
    ///     *field.downcast_mut::<i32>().unwrap() = 42;
    /// }
    ///
    /// assert_eq!(ts.a, 42);
    /// ```
    fn field_mut(&mut self, name: &str) -> Option<&mut dyn Reflect>;

    /// Returns a reference to the value of the field with index `index` as a
    /// `&dyn Reflect`.
    ///
    /// Returns `None` if `index` is out of bounds.
    ///
    /// If the field type is known, can use `<dyn Struct>::field_at_as` instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::Struct};
    /// #[derive(Reflect)]
    /// struct Foo{ a: i32, b: bool };
    ///
    /// let ts = Foo{ a: 1, b: true };
    ///
    /// assert!(ts.field_at(0).is_some());
    /// assert!(ts.field_at(2).is_none());
    /// ```
    fn field_at(&self, index: usize) -> Option<&dyn Reflect>;

    /// Returns a mutable reference to the value of the field with index `index`
    /// as a `&mut dyn Reflect`.
    ///
    /// Returns `None` if `index` is out of bounds.
    ///
    /// If the field type is known, can use `<dyn Struct>::field_at_mut_as` instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::Struct};
    /// #[derive(Reflect)]
    /// struct Foo{ a: i32, b: bool };
    ///
    /// let mut ts = Foo{ a: 1, b: true };
    ///
    /// if let Some(field) = ts.field_at_mut(0) {
    ///     *field.downcast_mut::<i32>().unwrap() = 42;
    /// }
    ///
    /// assert_eq!(ts.a, 42);
    /// ```
    fn field_at_mut(&mut self, index: usize) -> Option<&mut dyn Reflect>;

    /// Returns the name of the field with index `index`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::Struct};
    /// #[derive(Reflect)]
    /// struct Foo{ a: i32, b: bool };
    ///
    ///  let ts = Foo{ a: 1, b: true };
    ///
    /// assert_eq!(ts.name_at(0), Some("a"));
    /// assert_eq!(ts.name_at(2), None);
    /// ```
    fn name_at(&self, index: usize) -> Option<&str>;

    /// Returns the number of fields in the struct.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::Struct};
    /// #[derive(Reflect)]
    /// struct Foo{ a: i32, b: bool };
    ///
    /// let ts = Foo{ a: 1, b: true };
    ///
    /// assert_eq!(ts.field_len(), 2);
    /// ```
    fn field_len(&self) -> usize;

    /// Returns an iterator over the values of the struct's fields.
    ///
    /// The iterator yields references to each field in order,
    /// from index 0 to `field_len() - 1`.
    fn iter_fields(&self) -> StructFieldIter<'_>;

    /// Creates a new [`DynamicStruct`] from this struct.
    ///
    /// This is useful when you need a mutable, resizable version of a static struct.
    ///
    /// This function will replace all content with dynamic types, except for opaque types.
    ///
    /// # Panics
    ///
    /// Panics if inner items [`Reflect::to_dynamic`] failed.
    fn to_dynamic_struct(&self) -> DynamicStruct {
        let mut dynamic_struct = DynamicStruct::with_capacity(self.field_len());
        dynamic_struct.set_type_info(self.represented_type_info());
        for (i, val) in self.iter_fields().enumerate() {
            dynamic_struct.extend_boxed(self.name_at(i).unwrap().to_owned(), val.to_dynamic());
        }
        dynamic_struct
    }
}

impl Struct for DynamicStruct {
    #[inline]
    fn field(&self, name: &str) -> Option<&dyn Reflect> {
        self.field_indices.get(name).map(|index| &*self.fields[*index])
    }

    #[inline]
    fn field_mut(&mut self, name: &str) -> Option<&mut dyn Reflect> {
        self.field_indices.get(name).map(|index| &mut *self.fields[*index])
    }

    #[inline]
    fn field_at(&self, index: usize) -> Option<&dyn Reflect> {
        self.fields.get(index).map(Deref::deref)
    }

    #[inline]
    fn field_at_mut(&mut self, index: usize) -> Option<&mut dyn Reflect> {
        self.fields.get_mut(index).map(DerefMut::deref_mut)
    }

    #[inline]
    fn name_at(&self, index: usize) -> Option<&str> {
        self.field_names.get(index).map(AsRef::as_ref)
    }

    #[inline]
    fn field_len(&self) -> usize {
        self.fields.len()
    }

    #[inline]
    fn iter_fields(&self) -> StructFieldIter<'_> {
        StructFieldIter::new(self)
    }

    fn to_dynamic_struct(&self) -> DynamicStruct {
        DynamicStruct {
            info: self.represented_type_info(),
            fields: self.fields.iter().map(|val| val.to_dynamic()).collect(),
            field_names: self.field_names.clone(),
            field_indices: self.field_indices.clone(),
        }
    }
}

impl dyn Struct {
    /// Returns a typed reference to the field at the given field name.
    ///
    /// Returns `None` if:
    /// - The field does not exist.
    /// - The field cannot be downcast to type `T`
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{ops::Struct, Reflect};
    /// #[derive(Reflect)]
    /// struct Foo{ a: i32, b: &'static str };
    ///
    /// let foo = Foo{ a: 10_i32, b: "hello" };
    /// let foo_ref: &dyn Struct = &foo;
    ///
    /// assert_eq!(foo_ref.field_as::<i32>("a"), Some(&10));
    /// assert_eq!(foo_ref.field_as::<&str>("b"), Some(&"hello"));
    /// assert_eq!(foo_ref.field_as::<i32>("c"), None); // Out of bounds
    /// assert_eq!(foo_ref.field_as::<f64>("a"), None); // Wrong type
    /// ```
    #[inline]
    pub fn field_as<T: Reflect>(&self, name: &str) -> Option<&T> {
        self.field(name).and_then(<dyn Reflect>::downcast_ref)
    }

    /// Returns a typed mutable reference to the field at the given field name.
    ///
    /// Returns `None` if:
    /// - The field does not exist.
    /// - The field cannot be downcast to type `T`
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{ops::Struct, Reflect};
    /// #[derive(Reflect)]
    /// struct Foo{ a: i32, b: &'static str };
    ///
    /// let mut foo = Foo{ a: 10_i32, b: "hello" };
    /// let foo_ref: &mut dyn Struct = &mut foo;
    ///
    /// if let Some(field) = foo_ref.field_mut_as::<i32>("a") {
    ///     *field = 31;
    /// }
    ///
    /// assert_eq!(foo.a, 31);
    /// ```
    #[inline]
    pub fn field_mut_as<T: Reflect>(&mut self, name: &str) -> Option<&mut T> {
        self.field_mut(name).and_then(<dyn Reflect>::downcast_mut)
    }

    /// Returns a typed reference to the field at the given index.
    ///
    /// Returns `None` if:
    /// - The index is out of bounds
    /// - The field cannot be downcast to type `T`
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{ops::Struct, Reflect};
    /// #[derive(Reflect)]
    /// struct Foo{ a: i32, b: &'static str };
    ///
    /// let foo = Foo{ a: 10_i32, b: "hello" };
    /// let foo_ref: &dyn Struct = &foo;
    ///
    /// assert_eq!(foo_ref.field_at_as::<i32>(0), Some(&10));
    /// assert_eq!(foo_ref.field_at_as::<&str>(1), Some(&"hello"));
    /// assert_eq!(foo_ref.field_at_as::<i32>(2), None); // Out of bounds
    /// assert_eq!(foo_ref.field_at_as::<f64>(0), None); // Wrong type
    /// ```
    #[inline]
    pub fn field_at_as<T: Reflect>(&self, index: usize) -> Option<&T> {
        self.field_at(index).and_then(<dyn Reflect>::downcast_ref)
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
    /// # use voker_reflect::{ops::Struct, Reflect};
    /// #[derive(Reflect)]
    /// struct Foo{ a: i32, b: &'static str };
    ///
    /// let mut foo = Foo{ a: 10_i32, b: "hello" };
    /// let foo_ref: &mut dyn Struct = &mut foo;
    ///
    /// if let Some(field) = foo_ref.field_at_mut_as::<i32>(0) {
    ///     *field = 31;
    /// }
    ///
    /// assert_eq!(foo.a, 31);
    /// ```
    #[inline]
    pub fn field_at_mut_as<T: Reflect>(&mut self, index: usize) -> Option<&mut T> {
        self.field_at_mut(index).and_then(<dyn Reflect>::downcast_mut)
    }
}

// -----------------------------------------------------------------------------
// Struct Field Iterator

/// An iterator over the field values of a struct.
///
/// This is an [`ExactSizeIterator`] that yields references to each field
/// in the struct in order.
///
/// # Performance
///
/// The iterator uses [`Struct::field_at`] internally, which may have different
/// performance characteristics than iterating directly over a concrete struct type.
///
/// # Examples
///
/// ```
/// use voker_reflect::{Reflect, ops::{Struct, StructFieldIter}};
///
/// #[derive(Reflect)]
/// struct Foo{ a: i32, b: bool };
///
/// let ts = Foo{ a: 1, b: true };
/// let mut iter = StructFieldIter::new(&ts);
///
/// assert_eq!(iter.len(), 2);
/// assert_eq!(iter.next().and_then(|v| v.downcast_ref::<i32>()), Some(&1));
/// ```
pub struct StructFieldIter<'a> {
    struct_val: &'a dyn Struct,
    index: usize,
}

impl<'a> StructFieldIter<'a> {
    /// Creates a new iterator for the given struct.
    #[inline(always)]
    pub const fn new(value: &'a dyn Struct) -> Self {
        StructFieldIter {
            struct_val: value,
            index: 0,
        }
    }
}

impl<'a> Iterator for StructFieldIter<'a> {
    type Item = &'a dyn Reflect;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let value = self.struct_val.field_at(self.index);
        self.index += value.is_some() as usize;
        value
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let hint = self.struct_val.field_len() - self.index;
        (hint, Some(hint))
    }
}

impl ExactSizeIterator for StructFieldIter<'_> {}
impl FusedIterator for StructFieldIter<'_> {}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use super::DynamicStruct;
    use crate::info::TypePath;

    #[test]
    fn type_path() {
        assert!(DynamicStruct::type_path() == "voker_reflect::ops::DynamicStruct");
        assert!(DynamicStruct::module_path() == Some("voker_reflect::ops"));
        assert!(DynamicStruct::type_ident() == "DynamicStruct");
        assert!(DynamicStruct::type_name() == "DynamicStruct");
    }
}
