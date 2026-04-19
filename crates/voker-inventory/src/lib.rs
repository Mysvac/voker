//! CTOR based metadata collector.
//!
//! > Modified from <https://docs.rs/inventory/latest/inventory/>.
//! >
//! > - Date: 2024/04/02
//! > - Version: 0.3.24
//!
//! # Usage
//!
//! 1. Declare inventory support for a type through [`collect!`].
//! 2. Register values using [`submit!`].
//! 3. Iterate plugins with [`iter`].
//!
//! ```
//! use voker_inventory as inv;
//!
//! struct Flag(u64);
//!
//! inv::collect!(Flag);
//!
//! inv::submit!(Flag(0) => Flag);
//! inv::submit!(Flag(1) => Flag);
//!
//! for flag in inv::iter::<Flag>() {
//!     assert!(flag.0 == 0 || flag.0 == 1);
//! }
//! ```
//!
//! # Platform Support
//!
//! This crate supports Wasm, Windows, Linux, macOS, Android, iOS, and other
//! constructor-capable targets covered by the linker section attributes below.
//!
//! Notably, on Wasm you do not need to manually call `__wasm_call_ctors` as in
//! the upstream `inventory` crate. This implementation wraps that call inside
//! [`iter`], and runs it automatically on first use.
//!
//! # License
//!
//! Licensed under either of:
//! - Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) or
//!   <https://www.apache.org/licenses/LICENSE-2.0>)
//! - MIT license ([LICENSE-MIT](../LICENSE-MIT) or
//!   <https://opensource.org/licenses/MIT>)
//!
//! at your option.
#![expect(clippy::new_without_default, reason = "default is not const")]
#![expect(unsafe_code, reason = "pointer operation")]
#![no_std]

use core::cell::UnsafeCell;
use core::iter::FusedIterator;
use core::marker::PhantomData;
use core::ptr;
use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

#[cfg(debug_assertions)]
use core::any::TypeId;

const PENDING: usize = 0;
const RUNNING: usize = 1;
const COMPLETED: usize = 2;

struct Node {
    // Type-erased pointer to static data.
    data: *const (),
    // `next` is written only during submission, and exclusivity
    // is ensured by the atomic submission state. Before `submit`
    // completes, `next` is never read by iterators, so `next`
    // itself does not need to be atomic.
    next: UnsafeCell<Option<&'static Node>>,
    // Following the constraints in `voker-os`, we require the target
    // platform to support `AtomicPtr`, but not necessarily `AtomicU8`.
    // So we use `AtomicPtr` instead, maintain the same type size.
    state: AtomicUsize,
    #[cfg(debug_assertions)]
    type_id: TypeId, // Ensure type correctness in Debug mode.
}

/// Registry storage for one inventory type.
///
/// Internally this is a singly linked list head.
///
/// Reusing one [`Registry`] for multiple concrete types
/// is undefined behavior.
pub struct Registry {
    // Head pointer of the singly linked list for one concrete type.
    // New entries are inserted at the head (push-front).
    head: AtomicPtr<Node>,
}

/// A registrable inventory entry.
///
/// This type is intentionally public so users can opt into
/// manual submission:
///
/// - Create an item with the [`submit!`] macro using the
///   syntax `value => type as ident`.
/// - Check if already submitted with [`Item::is_submitted`].
/// - Manually submit with [`Item::submit`].
pub struct Item<T> {
    node: Node,
    _marker: PhantomData<T>,
}

/// Iterator over all submitted values of type `T`.
///
/// The iteration order is unspecified.
///
/// Construct this iterator through [`iter`].
pub struct Iter<T> {
    node: Option<&'static Node>,
    _marker: PhantomData<T>,
}

/// Marker trait for types that can participate in this inventory.
///
/// Prefer implementing this trait through [`collect!`].
///
/// # Examples
///
/// ```
/// # use voker_inventory::{Inventory, Registry};
/// struct Flag;
///
/// unsafe impl Inventory for Flag {
///     fn registry() -> &'static Registry {
///         static reg: Registry = Registry::new();
///         &reg
///     }
/// }
/// ```
///
/// # Safety
///
/// The returned registry must be dedicated to exactly one concrete type.
///
/// For example, avoid patterns that may cause the same [`Registry`] instance
/// to be shared across unrelated types, which would corrupt internal typing and
/// may trigger undefined behavior during iteration.
pub unsafe trait Inventory: Sync + Sized + 'static {
    fn registry() -> &'static Registry;
}

impl Registry {
    /// Creates an empty registry.
    pub const fn new() -> Self {
        Self {
            head: AtomicPtr::new(ptr::null_mut()),
        }
    }
}

unsafe impl<T: Inventory> Sync for Item<T> {}

impl<T: Inventory> Item<T> {
    /// Creates a registrable item that points to a `'static` value.
    pub const fn new(val: &'static T) -> Self {
        Self {
            _marker: PhantomData,
            node: Node {
                data: val as *const T as *const (),
                next: UnsafeCell::new(None),
                state: AtomicUsize::new(PENDING),
                #[cfg(debug_assertions)]
                type_id: TypeId::of::<T>(),
            },
        }
    }

    /// Returns whether this item has already been submitted.
    pub fn is_submitted(&self) -> bool {
        self.node.state.load(Ordering::Acquire) == COMPLETED
    }

    /// Submits this item into `T`'s registry.
    ///
    /// Repeated calls are idempotent.
    pub fn submit(&'static self) {
        use Ordering::{Acquire, Relaxed, Release};

        let node = &self.node;

        #[cfg(debug_assertions)]
        assert_eq!(
            node.type_id,
            TypeId::of::<T>(),
            "\n\
            ════════════════════════════════════════════════════════════════\n\
                Type Safety Violation in Inventory Registry                 \n\
            ════════════════════════════════════════════════════════════════\n\
                Operation: submit \n\
                Note: The submitted data type does not match the registry.\n\
                Expected type: `{}`(TypeId: {:?})\n\
                Found type:    `?`(TypeId: {:?})\n\
            ════════════════════════════════════════════════════════════════\n\
            ",
            core::any::type_name::<T>(),
            TypeId::of::<T>(),
            node.type_id,
        );

        if let Err(mut state) = node.state.compare_exchange(PENDING, RUNNING, Relaxed, Acquire) {
            while state != COMPLETED {
                core::hint::spin_loop();
                state = node.state.load(Acquire);
            }

            return;
        }

        let reg = <T as Inventory>::registry();
        let mut head = reg.head.load(Relaxed);

        loop {
            unsafe {
                *node.next.get() = head.as_ref();
            }

            let new_head = node as *const Node as *mut Node;

            if let Err(prev) = reg.head.compare_exchange(head, new_head, Release, Relaxed) {
                head = prev;
                continue;
            }

            node.state.store(COMPLETED, Release);
            return;
        }
    }
}

unsafe impl<T: Inventory> Sync for Iter<T> {}
unsafe impl<T: Inventory> Send for Iter<T> {}

impl<T: Inventory> Copy for Iter<T> {}

impl<T: Inventory> Clone for Iter<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Inventory> FusedIterator for Iter<T> {}

impl<T: Inventory> Iterator for Iter<T> {
    type Item = &'static T;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.node?;

        #[cfg(debug_assertions)]
        assert_eq!(
            node.type_id,
            TypeId::of::<T>(),
            "\n\
            ════════════════════════════════════════════════════════════════\n\
                Type Safety Violation in Inventory Registry                 \n\
            ════════════════════════════════════════════════════════════════\n\
                Operation: iter\n\
                Note: The same Registry may reused for different types. \n\
                Expected type: `{}`(TypeId: {:?})\n\
                Found type:    `?`(TypeId: {:?})\n\
            ════════════════════════════════════════════════════════════════\n\
            ",
            core::any::type_name::<T>(),
            TypeId::of::<T>(),
            node.type_id,
        );

        let ptr = node.data as *const T;
        debug_assert!(ptr.is_aligned());

        self.node = unsafe { *node.next.get() };
        unsafe { Some(&*ptr) }
    }
}

/// Returns an iterator over all submitted `T` values.
///
/// The iteration order is unspecified.
///
/// # Example
///
/// ```no_run
/// use voker_inventory as inv;
///
/// struct Flag(u8);
/// inv::collect!(Flag);
/// inv::submit!(Flag(1) => Flag);
///
/// let _ = inv::iter::<Flag>().count();
/// ```
pub fn iter<T: Inventory>() -> Iter<T> {
    #[cfg(target_family = "wasm")]
    call_ctor_in_wasm();

    let reg = <T as Inventory>::registry();
    let head = reg.head.load(Ordering::Acquire);
    unsafe {
        Iter {
            node: head.as_ref(),
            _marker: PhantomData,
        }
    }
}

#[inline]
#[cfg(target_family = "wasm")]
fn call_ctor_in_wasm() {
    unsafe extern "C" {
        unsafe fn __wasm_call_ctors();
    }

    static ONCE_FLAG: AtomicUsize = AtomicUsize::new(PENDING);
    if ONCE_FLAG.load(Ordering::Acquire) != COMPLETED {
        unsafe {
            __wasm_call_ctors();
        }
        ONCE_FLAG.store(COMPLETED, Ordering::Release);
    }
}

/// Associates an inventory registry with the specified type.
///
/// This macro must be invoked in the same crate that defines the type.
///
/// # Example
///
/// ```
/// use voker_inventory as inv;
///
/// struct Flag;
/// inv::collect!(Flag);
/// ```
#[macro_export]
macro_rules! collect {
    ($ty:ty) => {
        #[expect(unsafe_code, reason = "Inventory is unsafe")]
        unsafe impl $crate::Inventory for $ty {
            #[inline]
            fn registry() -> &'static $crate::Registry {
                static REGISTRY: $crate::Registry = $crate::Registry::new();
                &REGISTRY
            }
        }
    };
}

/// Submits a value to the registry of a given type.
///
/// Supported forms:
/// - `submit!(value => Type)` creates a private static item.
/// - `submit!(value => Type as NAME)` creates a public named static [`Item`].
///
/// This macro is intended for module scope (outside function bodies).
///
/// # Example
///
/// ```
/// use voker_inventory as inv;
///
/// struct Flag(u8);
/// inv::collect!(Flag);
///
/// inv::submit!(Flag(1) => Flag);
/// inv::submit!(Flag(2) => Flag as FLAG_TWO);
///
/// let _ = inv::iter::<Flag>().count();
/// ```
#[macro_export]
macro_rules! submit {
    ($value:expr => $ty:ty as $ident:ident) => {
        pub static $ident: $crate::Item<$ty> = {
            static __VALUE__: $ty = $value;
            <$crate::Item<$ty>>::new(&__VALUE__)
        };

        const _: () = {
            $crate::__call_ctor!($ident, $ty);
        };
    };
    ($value:expr => $ty:ty) => {
        const _: () = {
            static __ITEM__: $crate::Item<$ty> = {
                static __VALUE__: $ty = $value;
                <$crate::Item<$ty>>::new(&__VALUE__)
            };

            $crate::__call_ctor!(__ITEM__, $ty);
        };
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __call_ctor {
    ($ident:ident, $ty:ty) => {
        #[cfg_attr(
            any(target_os = "linux", target_os = "android"),
            unsafe(link_section = ".text.startup")
        )]
        unsafe extern "C" fn __ctor() {
            <$crate::Item<$ty>>::submit(&$ident);
        }

        // Linux/ELF: https://www.exploit-db.com/papers/13234
        //
        // macOS: https://blog.timac.org/2016/0716-constructor-and-destructor-attributes/
        //
        // Windows: https://www.cnblogs.com/sunkang/archive/2011/05/24/2055635.html
        // What is `.CRT$XCU` ?: 'I'=C init, 'C'=C++ init, 'P'=Pre-terminators and 'T'=Terminators
        #[used]
        #[cfg_attr(windows, unsafe(link_section = ".CRT$XCU"))]
        #[cfg_attr(
            any(target_os = "macos", target_os = "ios", target_os = "tvos",),
            unsafe(link_section = "__DATA,__mod_init_func,mod_init_funcs")
        )]
        #[cfg_attr(
            any(
                target_family = "wasm",
                target_os = "linux",
                target_os = "android",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "fuchsia",
                target_os = "illumos",
                target_os = "netbsd",
                target_os = "openbsd",
                target_os = "redox",
                target_os = "solaris",
                target_os = "haiku",
                target_os = "vxworks",
                target_os = "nto",
                target_os = "none",
            ),
            unsafe(link_section = ".init_array")
        )]
        static __CTOR: unsafe extern "C" fn() = __ctor;
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestValue(u32);
    collect!(TestValue);

    #[test]
    fn is_submitted() {
        submit!(TestValue(20) => TestValue as ITEM);
        assert!(ITEM.is_submitted());
    }

    #[test]
    fn submit_and_iter() {
        submit!(TestValue(1) => TestValue);
        submit!(TestValue(2) => TestValue);
        submit!(TestValue(3) => TestValue);

        assert!(iter::<TestValue>().any(|it| it.0 == 1));
        assert!(iter::<TestValue>().any(|it| it.0 == 2));
        assert!(iter::<TestValue>().any(|it| it.0 == 3));
        assert!(!iter::<TestValue>().any(|it| it.0 == 4));
    }
}
