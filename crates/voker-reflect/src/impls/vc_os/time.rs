use crate::derive::impl_reflect_opaque;

impl_reflect_opaque!(::voker_os::time::Instant(
    Debug, Clone, Hash, PartialEq, PartialOrd
));
