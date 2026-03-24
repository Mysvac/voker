//! Provide some utilities for implementing reflection traits.
//!
//! - [`concat`]: An efficient string concatenation function.
//! - [`NonGenericTypeInfoCell`]: Used to implement [`Typed`] for non-generic types.
//! - [`GenericTypePathCell`]: Used to implement [`TypePath`] for generic types.
//! - [`GenericTypeInfoCell`]: Used to implement [`Typed`] for generic types.
//! - `xxx_apply`: Used to implement [`Reflect::apply`] (e.g. [`array_apply`]).
//! - `xxx_hash`: Used to implement [`Reflect::reflect_hash`] (e.g. [`array_hash`]).
//! - `xxx_debug`: Used to implement [`Reflect::reflect_debug`] (e.g. [`array_debug`]).
//! - `xxx_eq`: Used to implement [`Reflect::reflect_eq`] (e.g. [`array_eq`]).
//! - `xxx_cmp`: Used to implement [`Reflect::reflect_cmp`] (e.g. [`array_cmp`]).
//!
//! ## Implemented Menu
//!
//! - basic:
//!     - `i8`-`i128`, `u8`-`u128`, `isize`, `usize`, `f32`, `f64`
//!     - `()`, `(P0,)`, `(P0, P1, ...)`. the num of P <= 12
//!     - `[T; N]`
//!     - `&'static str`, `String`
//! - core:
//!     - `Atomic`: Ordering, I8-I64, U8-U64, Isize, Usize (without Ptr)
//!     - `NonZero`: I8-I128, U8-U128, Isize, Usize, `Wrapping`, `Saturating`
//!     - `core::any::TypeId`
//!     - `PhantomData<T>`, T implemted `TypePath`.
//!     - `ops`: Range, Bound, RangeFull, RangeToInclusive, RangeTo, RangeFrom, RangeInclusive
//!     - `Option<T>` , `Result<T, E>`
//!     - `&'static core::panic::Location<'static>`
//!     - `core::time::Duration`
//! - alloc:
//!     - `String`, `Vec<T>`, `VecDeque<T>`
//!     - `Cow<'static, str>`, `Cow<'static, [T]>`
//!     - `BTreeMap<K, V>`, `BTreeSet<T>`
//!     - `Arc` (without `Box`)
//! - std: ("std" feature)
//!     - `OsString` `PathBuf`
//!     - `HashMap` `HashSet`
//!- voker_utils:
//!     - `Hashed` `HashMap` `HashSet`
//!     - `hashbrown::HashMap` `hashbrown::HashSet`
//!     - `fastvec::StackVec` `fastvec::AutoVec`
//! - voker_os:
//!     - `time::Instant`
//!
//! [`concat`]: crate::impls::concat
//! [`Reflect::reflect_cmp`]: crate::Reflect::reflect_cmp
//! [`Reflect::reflect_eq`]: crate::Reflect::reflect_eq
//! [`Reflect::reflect_debug`]: crate::Reflect::reflect_debug
//! [`Reflect::reflect_hash`]: crate::Reflect::reflect_hash
//! [`Reflect::apply`]: crate::Reflect::apply
//! [`TypePath`]: crate::info::TypePath
//! [`Typed`]: crate::info::Typed

// -----------------------------------------------------------------------------
// Modules

mod cell;
mod utils;

mod alloc;
mod core;
mod native;
mod vc_os;
mod vc_utils;

crate::cfg::std! { mod std; }

// -----------------------------------------------------------------------------
// Exports

pub use cell::{GenericTypeInfoCell, GenericTypePathCell, NonGenericTypeInfoCell};

pub use utils::*;

/// An efficient string concatenation function.
///
/// This is usually used for the implementation of `TypePath`.
///
/// # Example
///
/// ```
/// use voker_reflect::impls;
///
/// let s = impls::concat(&["module", "::", "name", "<", "T" , ">"]);
///
/// assert_eq!(s.capacity(), 15);
/// ```
///
/// Inline is prohibited here to reduce compilation time.
#[inline(never)]
pub fn concat(arr: &[&str]) -> ::alloc::string::String {
    let mut len = 0usize;
    for &item in arr {
        len += item.len();
    }
    let mut res = ::alloc::string::String::with_capacity(len);
    for &item in arr {
        res.push_str(item);
    }
    res
}
