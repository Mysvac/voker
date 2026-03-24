use crate::derive::impl_reflect_opaque;

impl_reflect_opaque!(::core::num::NonZeroI128(serde, clone, debug, hash, eq, cmp));
impl_reflect_opaque!(::core::num::NonZeroU128(serde, clone, debug, hash, eq, cmp));
impl_reflect_opaque!(::core::num::NonZeroIsize(
    serde, clone, debug, hash, eq, cmp
));
impl_reflect_opaque!(::core::num::NonZeroUsize(
    serde, clone, debug, hash, eq, cmp
));
impl_reflect_opaque!(::core::num::NonZeroI64(serde, clone, debug, hash, eq, cmp));
impl_reflect_opaque!(::core::num::NonZeroU64(serde, clone, debug, hash, eq, cmp));
impl_reflect_opaque!(::core::num::NonZeroU32(serde, clone, debug, hash, eq, cmp));
impl_reflect_opaque!(::core::num::NonZeroI32(serde, clone, debug, hash, eq, cmp));
impl_reflect_opaque!(::core::num::NonZeroI16(serde, clone, debug, hash, eq, cmp));
impl_reflect_opaque!(::core::num::NonZeroU16(serde, clone, debug, hash, eq, cmp));
impl_reflect_opaque!(::core::num::NonZeroU8(serde, clone, debug, hash, eq, cmp));
impl_reflect_opaque!(::core::num::NonZeroI8(serde, clone, debug, hash, eq, cmp));
impl_reflect_opaque!(::core::num::Wrapping<T: Clone + Send + Sync>(clone));
impl_reflect_opaque!(::core::num::Saturating<T: Clone + Send + Sync>(clone));
