use alloc::collections::BTreeSet;

use voker_os::sync::{Mutex, PoisonError};

use crate::component::{ComponentHook, ComponentId};

/// Intern pool for deduplicating small immutable identifier slices.
///
/// This avoids repeatedly allocating equivalent `'static` slices used by
/// archetype/component metadata. Identical slice contents are reused whenever
/// possible.
///
/// The pool intentionally leaks accepted slices for process-lifetime reuse.
pub struct SlicePool;

macro_rules! define_methods {
    ($name:ident, $ty:ty) => {
        pub fn $name(idents: &[$ty]) -> &'static [$ty] {
            // SlicePool is actually only used on the main thread.
            // So `Mutex` is faster then `RwLock`.
            static POOL: Mutex<BTreeSet<&[$ty]>> = Mutex::new(BTreeSet::new());

            if idents.is_empty() {
                return &[];
            }

            let guard = POOL.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(&idents) = guard.get(idents) {
                return idents;
            }
            ::core::mem::drop(guard);

            // Duplicate leak same slice is possible, but it's rare and acceptable.
            let slice: &[$ty] = pool::leak(idents);
            POOL.lock().unwrap_or_else(PoisonError::into_inner).insert(slice);
            slice
        }
    };
}

impl SlicePool {
    define_methods!(component, ComponentId);
    define_methods!(component_hook, (ComponentId, ComponentHook));
}

mod pool {
    use voker_os::sync::{Mutex, PoisonError};
    use voker_utils::extra::PagePool;

    /// A wrapper around `PagePool`.
    struct MemoryPool(PagePool<2048>);

    unsafe impl Sync for MemoryPool {}
    unsafe impl Send for MemoryPool {}

    static IDENT_POOL: Mutex<MemoryPool> = Mutex::new(MemoryPool(PagePool::new()));

    /// Similar to [`Box::leak`](alloc::boxed::Box), but backed by a page pool.
    ///
    /// Returned slices are process-lifetime allocations intended for stable,
    /// shared metadata identities.
    pub fn leak<T: Copy>(idents: &[T]) -> &'static [T] {
        let guard = IDENT_POOL.lock().unwrap_or_else(PoisonError::into_inner);
        unsafe {
            let slice: &[T] = guard.0.alloc_slice(idents);
            core::mem::transmute(slice)
        }
    }
}
