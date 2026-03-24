use crate::derive::impl_reflect_opaque;

impl_reflect_opaque!(bool(full));
impl_reflect_opaque!(char(full));
impl_reflect_opaque!(u8(full));
impl_reflect_opaque!(u16(full));
impl_reflect_opaque!(u32(full));
impl_reflect_opaque!(u64(full));
impl_reflect_opaque!(u128(full));
impl_reflect_opaque!(usize(full));
impl_reflect_opaque!(i8(full));
impl_reflect_opaque!(i16(full));
impl_reflect_opaque!(i32(full));
impl_reflect_opaque!(i64(full));
impl_reflect_opaque!(i128(full));
impl_reflect_opaque!(isize(full));
impl_reflect_opaque!(f32(serde, default, clone, debug, eq, cmp));
impl_reflect_opaque!(f64(serde, default, clone, debug, eq, cmp));
