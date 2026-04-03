// -----------------------------------------------------------------------------
// Modules

mod from_reflect;
mod reflect;

// -----------------------------------------------------------------------------
// Internal API

pub(crate) use reflect::impl_reflect_cast_fn;

// -----------------------------------------------------------------------------
// Exports

pub use from_reflect::FromReflect;
pub use reflect::Reflect;

// -----------------------------------------------------------------------------
// reflect_hasher

use core::hash::BuildHasher;
use voker_utils::hash::{FixedHashState, FixedHasher};

/// A Fixed Hasher for [`Reflect::reflect_hash`] implementation.
///
/// # Examples
///
/// ```
/// use core::hash::{Hash, Hasher};
/// fn fixed_hash<T: Hash>(val: &T) -> u64 {
///     let mut hasher = voker_reflect::reflect_hasher();
///     val.hash(&mut hasher);
///     hasher.finish()
/// }
/// # let _ = fixed_hash(&1);
/// ```
///
/// See [`FixedHashState`](voker_utils::hash::FixedHashState) for details.
#[inline(always)]
pub fn reflect_hasher() -> FixedHasher {
    FixedHashState.build_hasher()
}
