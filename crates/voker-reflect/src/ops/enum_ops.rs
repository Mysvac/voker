use alloc::borrow::{Cow, ToOwned};
use alloc::boxed::Box;
use alloc::string::String;
use core::cmp::Ordering;
use core::fmt;

use super::{impl_dynamic_type_info, impl_dynamic_type_path};
use crate::Reflect;
use crate::info::{TypeInfo, VariantKind};
use crate::ops::{ApplyError, ReflectCloneError};
use crate::ops::{DynamicStruct, DynamicTuple, DynamicVariant};
use crate::ops::{Struct, Tuple, VariantFieldIter};
use crate::reflection::impl_reflect_cast_fn;

// -----------------------------------------------------------------------------
// Dynamic Enum

/// A dynamic representation of an enum, allows for enums to be configured at runtime.
///
/// Just as Rust enumeration can only be one value at a time,
/// `DynamicEnum` can only storage one variant data.
///
/// # Type Information
///
/// Dynamic types are special in that their `TypeInfo` is [`OpaqueInfo`](crate::info::OpaqueInfo),
/// but other APIs behave like the represented type, such as [`reflect_kind`] and [`reflect_ref`].
///
/// A `DynamicEnum` can optionally represent a specific enum type through its
/// [`represented_type_info`]. When set, this allows the dynamic enum to be treated
/// as if it were a specific static enum type for reflection purposes.
///
/// # Example
///
/// ```
/// # use voker_reflect::{Reflect, ops::{DynamicEnum, DynamicVariant}};
///
/// // The original enum value
/// let mut value: Option<usize> = Some(123);
///
/// // Create a DynamicEnum to represent the new value
/// let mut dyn_enum = DynamicEnum::new(
///   1, "None",
///   DynamicVariant::Unit
/// );
///
/// // Apply the DynamicEnum as a patch to the original value
/// value.apply(dyn_enum.as_reflect());
///
/// // Tada!
/// assert_eq!(None, value);
/// ```
///
/// [`represented_type_info`]: Reflect::represented_type_info
/// [`reflect_kind`]: crate::Reflect::reflect_kind
/// [`reflect_ref`]: crate::Reflect::reflect_ref
pub struct DynamicEnum {
    info: Option<&'static TypeInfo>,
    variant_index: usize,
    variant_name: Cow<'static, str>,
    variant: DynamicVariant,
}

impl_dynamic_type_path!(DynamicEnum);
impl_dynamic_type_info!(DynamicEnum);

impl DynamicEnum {
    /// Create a new [`TypeInfo`] to represent an enum at runtime.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::{DynamicEnum, DynamicVariant}};
    /// let mut dynamic_option = DynamicEnum::new(1, "None", ());
    /// ```
    #[inline]
    pub fn new<I, V>(index: usize, name: I, variant: V) -> Self
    where
        I: Into<Cow<'static, str>>,
        V: Into<DynamicVariant>,
    {
        Self {
            info: None,
            variant_index: index,
            variant_name: name.into(),
            variant: variant.into(),
        }
    }

    /// Sets the [`TypeInfo`] that this dynamic enum represents.
    ///
    /// When set, [`Reflect::represented_type_info`] will return this information,
    /// allowing the dynamic enum to be treated as if it were a specific static enum type.
    ///
    /// # Panics
    ///
    /// Panics if `info` is `Some` but does not contain enum type information.
    #[inline]
    pub const fn set_type_info(&mut self, info: Option<&'static TypeInfo>) {
        match info {
            Some(info) => {
                assert!(info.kind().is_enum(), "`TypeInfo` mismatched.");
                self.info = Some(info);
            }
            None => {
                self.info = None;
            }
        }
    }

    /// Set the current enum variant represented by this struct.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::{DynamicEnum, DynamicVariant, DynamicTuple}};
    /// let mut dynamic_option = DynamicEnum::new(1, "None", ());
    ///
    /// let mut val = DynamicTuple::new();
    /// val.extend(1);
    ///
    /// dynamic_option.set_variant(0, "Some", val);
    /// ```
    #[inline]
    pub fn set_variant<I, V>(&mut self, index: usize, name: I, variant: V)
    where
        I: Into<Cow<'static, str>>,
        V: Into<DynamicVariant>,
    {
        self.variant_name = name.into();
        self.variant_index = index;
        self.variant = variant.into();
    }

    /// Get a reference to the [`DynamicVariant`] contained in `self`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::{DynamicEnum, DynamicVariant, DynamicTuple}};
    /// let mut val = DynamicTuple::new();
    /// val.extend(1);
    ///
    /// let dynamic_option = DynamicEnum::new(0, "Some", val);
    ///
    /// if let DynamicVariant::Tuple(tuple) = dynamic_option.variant() {
    ///     /* ... */
    /// }
    /// ```
    #[inline]
    pub fn variant(&self) -> &DynamicVariant {
        &self.variant
    }

    /// Get a mutable reference to the [`DynamicVariant`] contained in `self`.
    ///
    /// Using the mut reference to switch to a different variant will ___not___ update the
    /// internal tracking of the variant name and index.
    ///
    /// If you want to switch variants, can use [`DynamicEnum::set_variant`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::{DynamicEnum, DynamicVariant, DynamicTuple}};
    /// let mut val = DynamicTuple::new();
    /// val.extend(1);
    ///
    /// let mut dynamic_option = DynamicEnum::new(0, "Some", val);
    ///
    /// if let DynamicVariant::Tuple(tuple) = dynamic_option.variant_mut() {
    ///     /* ... */
    /// }
    /// ```
    #[inline]
    pub fn variant_mut(&mut self) -> &mut DynamicVariant {
        &mut self.variant
    }

    /// Create a [`DynamicEnum`] from an existing one.
    ///
    /// This is functionally the same as [`DynamicEnum::from_ref`] except this takes an owned value.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::ops::{DynamicEnum, Enum};
    ///
    /// let dyn_enum = DynamicEnum::from(Some(10));
    /// assert_eq!(dyn_enum.variant_name(), "Some");
    /// ```
    #[inline]
    pub fn from<TEnum: Enum>(value: TEnum) -> Self {
        Self::from_ref(&value)
    }

    /// Create a [`DynamicEnum`] from an existing one.
    ///
    /// This is functionally the same as [`DynamicEnum::from`] except this takes a reference.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::ops::{DynamicEnum, Enum};
    ///
    /// let dyn_enum = DynamicEnum::from_ref(&Some(10));
    /// assert_eq!(dyn_enum.variant_name(), "Some");
    /// ```
    #[inline(never)]
    pub fn from_ref<TEnum: Enum + ?Sized>(value: &TEnum) -> Self {
        let mut dyn_enum = match value.variant_kind() {
            VariantKind::Unit => DynamicEnum::new(
                value.variant_index(),
                value.variant_name().to_owned(),
                DynamicVariant::Unit,
            ),
            VariantKind::Tuple => {
                let mut data = DynamicTuple::with_capacity(value.field_len());
                for field in value.iter_fields() {
                    data.extend_boxed(field.value().to_dynamic());
                }
                DynamicEnum::new(
                    value.variant_index(),
                    value.variant_name().to_owned(),
                    DynamicVariant::Tuple(data),
                )
            }
            VariantKind::Struct => {
                let mut data = DynamicStruct::with_capacity(value.field_len());
                for field in value.iter_fields() {
                    let name = field.name().unwrap();
                    data.extend_boxed(name.to_owned(), field.value().to_dynamic());
                }
                DynamicEnum::new(
                    value.variant_index(),
                    value.variant_name().to_owned(),
                    DynamicVariant::Struct(data),
                )
            }
        };

        dyn_enum.set_type_info(value.represented_type_info());
        dyn_enum
    }
}

impl Reflect for DynamicEnum {
    impl_reflect_cast_fn!(Enum);

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
        Box::new(<Self as Enum>::to_dynamic_enum(self))
    }

    #[inline]
    fn reflect_clone(&self) -> Result<Box<dyn Reflect>, ReflectCloneError> {
        Ok(Box::new(<Self as Enum>::to_dynamic_enum(self)))
    }

    fn apply(&mut self, value: &dyn Reflect) -> Result<(), ApplyError> {
        if let Some(y) = crate::impls::enum_apply(self, value)? {
            let dyn_variant = match y.variant_kind() {
                VariantKind::Unit => DynamicVariant::Unit,
                VariantKind::Tuple => {
                    let mut dyn_tuple = DynamicTuple::with_capacity(y.field_len());
                    for y_field in y.iter_fields() {
                        dyn_tuple.extend_boxed(y_field.value().to_dynamic());
                    }
                    DynamicVariant::Tuple(dyn_tuple)
                }
                VariantKind::Struct => {
                    let mut dyn_struct = DynamicStruct::with_capacity(y.field_len());
                    for y_field in y.iter_fields() {
                        dyn_struct.extend_boxed(
                            y_field.name().unwrap().to_owned(),
                            y_field.value().to_dynamic(),
                        );
                    }
                    DynamicVariant::Struct(dyn_struct)
                }
            };

            self.set_variant(y.variant_index(), y.variant_name().to_owned(), dyn_variant);
        }
        Ok(())
    }

    #[inline]
    fn reflect_eq(&self, other: &dyn Reflect) -> Option<bool> {
        crate::impls::enum_eq(self, other)
    }

    #[inline]
    fn reflect_cmp(&self, other: &dyn Reflect) -> Option<Ordering> {
        crate::impls::enum_cmp(self, other)
    }

    #[inline]
    fn reflect_hash(&self) -> Option<u64> {
        crate::impls::enum_hash(self)
    }

    #[inline]
    fn reflect_debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DynamicEnum(")?;
        crate::impls::enum_debug(self, f)?;
        write!(f, ")")
    }
}

impl fmt::Debug for DynamicEnum {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.reflect_debug(f)
    }
}

// -----------------------------------------------------------------------------
// Enum trait

/// A trait used to power [enum-like] operations via [reflection].
///
/// This allows enums to be processed and modified dynamically at runtime without
/// necessarily knowing the actual type.
///
/// Enums are much more complex than their struct counterparts.
/// As a result, users will need to be mindful of conventions, considerations,
/// and complications when working with this trait.
///
/// # Variants
///
/// An enum is a set of choices called _variants_.
/// An instance of an enum can only exist as one of these choices at any given time.
/// Consider Rust's [`Option<T>`]. It's an enum with two variants: [`None`] and [`Some`].
/// If you're `None`, you can't be `Some` and vice versa.
///
/// > ⚠️ **This is very important:**
/// > The [`Enum`] trait represents an enum _as one of its variants_.
/// > It does not represent the entire enum since that's not true to how enums work.
///
/// Variants come in a few [flavors](VariantKind):
///
/// | Variant Type | Syntax                         |
/// | ------------ | ------------------------------ |
/// | Unit         | `MyEnum::Foo`                  |
/// | Tuple        | `MyEnum::Foo( i32, i32 )`      |
/// | Struct       | `MyEnum::Foo{ value: String }` |
///
/// As you can see, a unit variant contains no fields, while tuple and struct variants
/// can contain zero, one or more fields.
///
/// The fields in a tuple variant is defined by their _order_ within the variant.
/// Index `0` represents the first field in the variant and so on.
/// Fields in struct variants (excluding tuple structs), on the other hand, are
/// represented by a _name_.
///
/// # Implementation
///
/// > 💡 This trait can be automatically implemented using [`#[derive(Reflect)]`](crate::derive::Reflect)
/// > on an enum definition.
///
/// Despite the fact that enums can represent multiple states, traits only exist in one state
/// and must be applied to the entire enum rather than a particular variant.
/// Because of this limitation, the [`Enum`] trait must not only _represent_ any of the
/// three variant types, but also define the _methods_ for all three as well.
///
/// What does this mean? It means that even though a unit variant contains no fields, a
/// representation of that variant using the [`Enum`] trait will still contain methods for
/// accessing fields!
/// Again, this is to account for _all three_ variant types.
///
/// We recommend using the built-in [`#[derive(Reflect)]`](crate::derive::Reflect) macro to automatically handle all the
/// implementation details for you.
/// However, if you _must_ implement this trait manually, there are a few things to keep in mind...
///
/// ## Field Order
///
/// While tuple variants identify their fields by the order in which they are defined, struct
/// variants identify fields by their name.
/// However, both should allow access to fields by their defined order.
///
/// The reason all fields, regardless of variant type, need to be accessible by their order is
/// due to field iteration.
/// We need a way to iterate through each field in a variant, and the easiest way of achieving
/// that is through the use of field order.
///
/// The derive macro adds proper struct variant handling for [`Enum::name_at`]
/// and [`Enum::field_at[_mut]`](Enum::field_at) methods.
/// The first two methods are __required__ for all struct variant types.
/// By convention, implementors should also handle the last method as well, but this is not
/// a strict requirement.
///
/// ## Field Names
///
/// Implementors may choose to handle  [`Enum::name_at`], and
/// [`Enum::field[_mut]`](Enum::field) for tuple variants by considering stringified `usize`s to be
/// valid names (such as `"3"`).
/// This isn't wrong to do, but the convention set by the derive macro is that it isn't supported.
/// It's preferred that these strings be converted to their proper `usize` representations and
/// the [`Enum::field_at[_mut]`](Enum::field_at) methods be used instead.
///
/// [enum-like]: https://doc.rust-lang.org/book/ch06-01-defining-an-enum.html
/// [reflection]: crate
/// [`None`]: Option<T>::None
/// [`Some`]: Option<T>::Some
pub trait Enum: Reflect {
    /// Returns a reference to the value of the field (in the current variant) with the given name.
    ///
    /// For non-[`VariantKind::Struct`] variants, this should return `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::Enum};
    ///
    /// #[derive(Reflect)]
    /// enum Foo {
    ///     Data{ id: u32, data: u64 },
    ///     None,
    /// }
    ///
    /// let foo = Foo::None;
    /// assert!(foo.field("id").is_none());
    ///
    /// let foo = Foo::Data{ id: 0, data: 0 };
    /// assert!(foo.field("id").is_some());
    /// assert!(foo.field("ty").is_none());
    /// ```
    fn field(&self, name: &str) -> Option<&dyn Reflect>;

    /// Returns a reference to the value of the field (in the current variant) at the given index.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::Enum};
    ///
    /// #[derive(Reflect)]
    /// enum Foo {
    ///     Data{ id: u32, data: u64 },
    ///     None,
    /// }
    ///
    /// let foo = Foo::None;
    /// assert!(foo.field_at(0).is_none());
    ///
    /// let foo = Foo::Data{ id: 0, data: 0 };
    /// assert!(foo.field_at(0).is_some());
    /// assert!(foo.field_at(2).is_none());
    /// ```
    fn field_at(&self, index: usize) -> Option<&dyn Reflect>;

    /// Returns a mutable reference to the value of the field (in the current variant) with the given name.
    ///
    /// For non-[`VariantKind::Struct`] variants, this should return `None`.
    fn field_mut(&mut self, name: &str) -> Option<&mut dyn Reflect>;

    /// Returns a mutable reference to the value of the field (in the current variant) at the given index.
    fn field_at_mut(&mut self, index: usize) -> Option<&mut dyn Reflect>;

    // /// Returns the index of the field (in the current variant) with the given name.
    // ///
    // /// For non-[`VariantKind::Struct`] variants, this should return `None`.
    // fn index_of(&self, name: &str) -> Option<usize>;

    /// Returns the name of the field (in the current variant) with the given index.
    ///
    /// For non-[`VariantKind::Struct`] variants, this should return `None`.
    fn name_at(&self, index: usize) -> Option<&str>;

    /// Returns an iterator over the values of the current variant's fields.
    fn iter_fields(&self) -> VariantFieldIter<'_>;

    /// Returns the number of fields in the current variant.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::Enum};
    ///
    /// #[derive(Reflect)]
    /// enum Foo {
    ///     Data{ id: u32, data: u64 },
    ///     None,
    /// }
    ///
    /// let foo = Foo::None;
    /// assert_eq!(foo.field_len(), 0);
    ///
    /// let foo = Foo::Data{ id: 0, data: 0 };
    /// assert_eq!(foo.field_len(), 2);
    /// ```
    fn field_len(&self) -> usize;

    /// The name of the current variant.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::Enum};
    ///
    /// #[derive(Reflect)]
    /// enum Foo {
    ///     Data{ id: u32, data: u64 },
    ///     None,
    /// }
    ///
    /// let foo = Foo::None;
    /// assert_eq!(foo.variant_name(), "None");
    ///
    /// let foo = Foo::Data{ id: 0, data: 0 };
    /// assert_eq!(foo.variant_name(), "Data");
    /// ```
    fn variant_name(&self) -> &str;

    /// Returns the full path to the current variant.
    ///
    /// Note that this is **origin** type_path + variant_name,
    /// not represented type_path + variant_name.
    ///
    /// Therefore, unexpected results may be returned when using dynamic types.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::Enum};
    ///
    /// #[derive(Reflect)]
    /// #[type_path = "hello::Foo"]
    /// enum Foo {
    ///     Data{ id: u32, data: u64 },
    ///     None,
    /// }
    ///
    /// let foo = Foo::None;
    /// assert_eq!(foo.variant_path(), "hello::Foo::None");
    ///
    /// let foo = Foo::Data{ id: 0, data: 0 };
    /// assert_eq!(foo.variant_path(), "hello::Foo::Data");
    /// ```
    fn variant_path(&self) -> String {
        crate::impls::concat(&[self.reflect_type_path(), "::", self.variant_name()])
    }

    /// The index of the current variant.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::Enum};
    ///
    /// #[derive(Reflect)]
    /// enum Foo {
    ///     Data{ id: u32, data: u64 },
    ///     None,
    /// }
    ///
    /// let foo = Foo::None;
    /// assert_eq!(foo.variant_index(), 1);
    ///
    /// let foo = Foo::Data{ id: 0, data: 0 };
    /// assert_eq!(foo.variant_index(), 0);
    /// ```
    fn variant_index(&self) -> usize;

    /// The type of the current variant.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::Enum, info::VariantKind};
    ///
    /// #[derive(Reflect)]
    /// enum Foo {
    ///     Data{ id: u32, data: u64 },
    ///     Num(i32),
    ///     None,
    /// }
    ///
    /// let foo = Foo::None;
    /// assert_eq!(foo.variant_kind(), VariantKind::Unit);
    ///
    /// let foo = Foo::Data{ id: 0, data: 0 };
    /// assert_eq!(foo.variant_kind(), VariantKind::Struct);
    ///
    /// let foo = Foo::Num(0);
    /// assert_eq!(foo.variant_kind(), VariantKind::Tuple);
    /// ```
    fn variant_kind(&self) -> VariantKind;

    /// Creates a new [`DynamicEnum`] from this enum.
    #[inline]
    fn to_dynamic_enum(&self) -> DynamicEnum {
        DynamicEnum::from_ref(self)
    }
}

impl Enum for DynamicEnum {
    fn field(&self, name: &str) -> Option<&dyn Reflect> {
        if let DynamicVariant::Struct(data) = &self.variant {
            data.field(name)
        } else {
            None
        }
    }

    fn field_at(&self, index: usize) -> Option<&dyn Reflect> {
        match &self.variant {
            DynamicVariant::Tuple(data) => data.field(index),
            DynamicVariant::Struct(data) => data.field_at(index),
            DynamicVariant::Unit => None,
        }
    }

    fn field_mut(&mut self, name: &str) -> Option<&mut dyn Reflect> {
        if let DynamicVariant::Struct(data) = &mut self.variant {
            data.field_mut(name)
        } else {
            None
        }
    }

    fn field_at_mut(&mut self, index: usize) -> Option<&mut dyn Reflect> {
        match &mut self.variant {
            DynamicVariant::Tuple(data) => data.field_mut(index),
            DynamicVariant::Struct(data) => data.field_at_mut(index),
            DynamicVariant::Unit => None,
        }
    }

    fn name_at(&self, index: usize) -> Option<&str> {
        if let DynamicVariant::Struct(data) = &self.variant {
            data.name_at(index)
        } else {
            None
        }
    }

    #[inline]
    fn iter_fields(&self) -> VariantFieldIter<'_> {
        VariantFieldIter::new(self)
    }

    #[inline]
    fn field_len(&self) -> usize {
        match &self.variant {
            DynamicVariant::Unit => 0,
            DynamicVariant::Tuple(data) => data.field_len(),
            DynamicVariant::Struct(data) => data.field_len(),
        }
    }

    #[inline]
    fn variant_name(&self) -> &str {
        &self.variant_name
    }

    #[inline]
    fn variant_index(&self) -> usize {
        self.variant_index
    }

    #[inline]
    fn variant_kind(&self) -> VariantKind {
        match &self.variant {
            DynamicVariant::Unit => VariantKind::Unit,
            DynamicVariant::Tuple(..) => VariantKind::Tuple,
            DynamicVariant::Struct(..) => VariantKind::Struct,
        }
    }
}

impl dyn Enum {
    /// Returns a typed reference to the field at the given field name.
    ///
    /// Returns `None` if:
    /// - The enum variant is not Struct.
    /// - The field does not exist.
    /// - The field cannot be downcast to type `T`
    #[inline]
    pub fn field_as<T: Reflect>(&self, name: &str) -> Option<&T> {
        self.field(name).and_then(<dyn Reflect>::downcast_ref)
    }

    /// Returns a typed mutable reference to the field at the given field name.
    ///
    /// Returns `None` if:
    /// - The enum variant is not Struct.
    /// - The field does not exist.
    /// - The field cannot be downcast to type `T`
    #[inline]
    pub fn field_mut_as<T: Reflect>(&mut self, name: &str) -> Option<&mut T> {
        self.field_mut(name).and_then(<dyn Reflect>::downcast_mut)
    }

    /// Returns a typed reference to the field at the given index.
    ///
    /// Returns `None` if:
    /// - The enum variant is Unit.
    /// - The index is out of bounds
    /// - The field cannot be downcast to type `T`
    #[inline]
    pub fn field_at_as<T: Reflect>(&self, index: usize) -> Option<&T> {
        self.field_at(index).and_then(<dyn Reflect>::downcast_ref)
    }

    /// Returns a typed mutable reference to the field at the given index.
    ///
    /// Returns `None` if:
    /// - The enum variant is Unit.
    /// - The index is out of bounds
    /// - The field cannot be downcast to type `T`
    #[inline]
    pub fn field_at_mut_as<T: Reflect>(&mut self, index: usize) -> Option<&mut T> {
        self.field_at_mut(index).and_then(<dyn Reflect>::downcast_mut)
    }
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use super::DynamicEnum;
    use crate::info::TypePath;

    #[test]
    fn type_path() {
        assert!(DynamicEnum::type_path() == "voker_reflect::ops::DynamicEnum");
        assert!(DynamicEnum::type_ident() == "DynamicEnum");
        assert!(DynamicEnum::type_name() == "DynamicEnum");
        assert!(DynamicEnum::module_path() == Some("voker_reflect::ops"));
    }
}
