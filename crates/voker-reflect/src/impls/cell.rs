//! Containers for static storage of type information.
//!
//! This is usually used to implement [`Typed`] and [`TypePath`].
//!
//! [`Typed`]: crate::info::Typed
//! [`TypePath`]: crate::info::TypePath

use alloc::string::String;
use core::any::{Any, TypeId};

use voker_os::sync::{OnceLock, PoisonError, RwLock};
use voker_utils::extra::TypeIdMap;

use crate::info::TypeInfo;

// -----------------------------------------------------------------------------
// NonGenericTypeInfoCell

/// Container for static storage of non-generic type information.
///
/// This is usually used to implement [`Typed`](crate::info::Typed).
///
/// Notr: There is no `NonGenericTypePathCell` because it can be replaced
/// by a static string literal.
///
/// ## Example
///
/// ```ignore
/// #[derive(Reflect)]
/// #[reflect(Typed = false)]
/// struct A2 {
///     a: u32
/// }
///
/// impl Typed for A2 {
///     fn type_info() -> &'static TypeInfo {
///         static CELL: NonGenericTypeInfoCell = NonGenericTypeInfoCell::new();
///         CELL.get_or_init(||TypeInfo::Struct(
///             StructInfo::new::<A2>(&[
///                 NamedField::new::<u32>("a")
///             ])
///         ))
///     }
/// }
///
/// let info = A2::type_info().as_struct().unwrap();
/// assert_eq!(info.field("a").unwrap().type_path(), "u32");
/// assert_eq!(info.type_name(), "A2");
/// ```
pub struct NonGenericTypeInfoCell(OnceLock<&'static TypeInfo>);

impl NonGenericTypeInfoCell {
    /// Create a empty cell.
    ///
    /// See [`NonGenericTypeInfoCell`].
    #[inline]
    #[expect(
        clippy::new_without_default,
        reason = "default cannot be used in const context"
    )]
    pub const fn new() -> Self {
        Self(OnceLock::new())
    }

    /// Returns a reference to the `Info` stored in the cell.
    ///
    /// If there is no entry found, a new one will be generated from the given function.
    ///
    /// See [`NonGenericTypeInfoCell`].
    #[inline]
    pub fn get_or_init<F>(&self, f: F) -> &TypeInfo
    where
        F: FnOnce() -> TypeInfo,
    {
        self.0.get_or_init(|| pool::leak_info(f()))
    }
}

// -----------------------------------------------------------------------------
// GenericTypeInfoCell

/// Container for static storage of type information with generics.
///
/// ## Example
///
/// ```ignore
/// #[derive(Reflect)]
/// #[reflect(Typed = false)]
/// struct A3<T>(T);
///
/// impl<T: Typed + Reflect> Typed for A3<T> {
///     fn type_info() -> &'static TypeInfo {
///         static CELL: GenericTypeInfoCell = GenericTypeInfoCell::new();
///         CELL.get_or_insert::<Self>(||TypeInfo::TupleStruct(
///             TupleStructInfo::new::<A3<T>>(&[
///                 UnnamedField::new::<T>(0)
///             ])
///         ))
///     }
/// }
///
/// let info = <A3<u64>>::type_info().as_tuple_struct().unwrap();
/// assert_eq!(info.field_at(0).unwrap().type_path(), "u64");
/// assert_eq!(info.type_name(), "A3<u64>");
/// ```
pub struct GenericTypeInfoCell(RwLock<TypeIdMap<&'static TypeInfo>>);

/// Container for static storage of type path with generics.
///
/// ## Example
///
/// ```ignore
/// use voker_reflect::impls;
///
/// #[derive(Reflect)]
/// #[reflect(TypePath = false)]
/// enum A4<T>{
///     None,
///     Some(T),
/// }
///
/// impl<T: TypePath> TypePath for A4<T> {
///     fn type_path() -> &'static str {
///         static CELL: GenericTypePathCell = GenericTypePathCell::new();
///         CELL.get_or_insert::<Self>(||{
///             impls::concat(&["test::A4", "<", T::type_path() , ">"])
///         })
///     }
///     fn type_name() -> &'static str {
///         static CELL: GenericTypePathCell = GenericTypePathCell::new();
///         CELL.get_or_insert::<Self>(||{
///             impls::concat(&["A4", "<", T::type_name() , ">"])
///         })
///     }
///     fn type_ident() -> &'static str { "A4" }
/// }
///
/// assert_eq!(<A4<i32>>::type_path(), "test::A4<i32>");
/// assert_eq!(<A4<u8>>::type_name(), "A4<u8>");
/// ```
pub struct GenericTypePathCell(RwLock<TypeIdMap<&'static str>>);

macro_rules! impl_generic_cell {
    ($name:ty , $leak:ident : $data:ty , $ret: ty) => {
        impl $name {
            /// Create a empty cell.
            #[inline]
            #[expect(
                clippy::new_without_default,
                reason = "default cannot be used in const context"
            )]
            pub const fn new() -> Self {
                Self(RwLock::new(TypeIdMap::new()))
            }

            /// Returns a reference to the `Info` stored in the cell.
            ///
            /// This method will then return the correct `Info` reference for the given type `T`.
            /// If there is no entry found, a new one will be generated from the given function.
            #[inline(always)]
            pub fn get_or_insert<G: Any + ?Sized>(&self, f: impl FnOnce() -> $data) -> &$ret {
                // Separate to reduce code compilation times
                self.get_or_insert_by_type_id(TypeId::of::<G>(), f)
            }

            // Separate to reduce code compilation times
            #[inline(never)]
            fn get_or_insert_by_type_id(
                &self,
                type_id: TypeId,
                f: impl FnOnce() -> $data,
            ) -> &$ret {
                match self.get_by_type_id(type_id) {
                    Some(info) => info,
                    None => self.insert_by_type_id(type_id, f()),
                }
            }

            // Separate to reduce code compilation times
            #[inline(never)]
            fn get_by_type_id(&self, type_id: TypeId) -> Option<&$ret> {
                self.0
                    .read()
                    .unwrap_or_else(PoisonError::into_inner)
                    .get(&type_id)
                    .copied()
            }

            // Separate to reduce code compilation times
            #[cold]
            #[inline(never)]
            fn insert_by_type_id(&self, type_id: TypeId, value: $data) -> &$ret {
                self.0
                    .write()
                    .unwrap_or_else(PoisonError::into_inner)
                    .get_or_insert(type_id, || pool::$leak(value))
            }
        }
    };
}

impl_generic_cell!(GenericTypeInfoCell , leak_info : TypeInfo , TypeInfo);
impl_generic_cell!(GenericTypePathCell , leak_path : String , str);

// -----------------------------------------------------------------------------
// pool

#[expect(unsafe_code, reason = "sealed implementation")]
mod pool {
    use alloc::string::String;
    use core::alloc::Layout;

    use voker_os::sync::{Mutex, PoisonError};
    use voker_utils::extra::PagePool;

    use crate::info::TypeInfo;

    /// A wrapper around `PagePool`.
    ///
    /// Since we only wrap it with `Mutex` as a static variable, marking it as
    /// `Sync` and `Send` is safe.
    ///
    /// Since a `TypeInfo` is about 128/144 bytes, `PAGE_SIZE = 2048` is appropriate.
    struct MemoryPool(PagePool<2048>);

    unsafe impl Sync for MemoryPool {}
    unsafe impl Send for MemoryPool {}

    static INFO_POOL: Mutex<MemoryPool> = Mutex::new(MemoryPool(PagePool::new()));
    static PATH_POOL: Mutex<MemoryPool> = Mutex::new(MemoryPool(PagePool::new()));

    /// Similar to [`Box::leak`](alloc::boxed::Box), but leaking in memory pool.
    pub fn leak_info(value: TypeInfo) -> &'static TypeInfo {
        let ptr = INFO_POOL
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .0
            .alloc(Layout::new::<TypeInfo>());
        let ptr = ptr.as_ptr() as *mut TypeInfo;

        unsafe {
            core::ptr::write(ptr, value);
            &*ptr
        }
    }

    /// Similar to [`Box::leak`](alloc::boxed::Box), but leaking in memory pool.
    pub fn leak_path(value: String) -> &'static str {
        let guard = PATH_POOL.lock().unwrap_or_else(PoisonError::into_inner);
        unsafe {
            let ref_str = guard.0.alloc_str(&value);
            core::mem::transmute::<&str, &'static str>(ref_str)
        }
    }
}
