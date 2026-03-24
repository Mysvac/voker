/// A container for [`ConstParamInfo`](crate::info::ConstParamInfo).
///
/// Internal type, users should not use this type directly.
///
/// The only allowed types of const parameters are
/// u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, char and bool.
///
/// ```
/// # use voker_reflect::info::ConstParamData;
/// let x: ConstParamData = 7i32.into();
///
/// let y = TryInto::<i32>::try_into(x).unwrap();
/// assert_eq!(y, 7i32);
/// ```
///
/// See: <https://doc.rust-lang.org/reference/items/generics.html>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstParamData {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    Usize(usize),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),
    Isize(isize),
    Char(char),
    Bool(bool),
}

macro_rules! impl_from_fn {
    ($ty:ident, $kind:ident) => {
        impl From<$ty> for ConstParamData {
            #[inline(always)]
            fn from(value: $ty) -> Self {
                Self::$kind(value)
            }
        }
        impl TryFrom<ConstParamData> for $ty {
            type Error = ();
            #[inline]
            fn try_from(value: ConstParamData) -> Result<Self, Self::Error> {
                match value {
                    ConstParamData::$kind(v) => Ok(v),
                    _ => Err(()),
                }
            }
        }
    };
}

impl_from_fn!(u8, U8);
impl_from_fn!(u16, U16);
impl_from_fn!(u32, U32);
impl_from_fn!(u64, U64);
impl_from_fn!(u128, U128);
impl_from_fn!(usize, Usize);
impl_from_fn!(i8, I8);
impl_from_fn!(i16, I16);
impl_from_fn!(i32, I32);
impl_from_fn!(i64, I64);
impl_from_fn!(i128, I128);
impl_from_fn!(isize, Isize);
impl_from_fn!(char, Char);
impl_from_fn!(bool, Bool);
