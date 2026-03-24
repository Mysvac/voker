use alloc::alloc as malloc;
use core::alloc::Layout;
use core::fmt::Debug;
use core::num::NonZeroUsize;
use core::ptr::{self, NonNull};

use voker_ptr::{OwningPtr, Ptr, PtrMut};

use crate::borrow::{UntypedMut, UntypedRef};
use crate::resource::{Resource, ResourceInfo};
use crate::tick::{Tick, TicksMut, TicksRef};
use crate::utils::{DebugName, Dropper};

// -----------------------------------------------------------------------------
// Drop Guard

/// Drop guard that aborts the process if a resource's drop implementation panics.
struct AbortOnDropFail;

impl Drop for AbortOnDropFail {
    #[cold]
    #[inline(never)]
    fn drop(&mut self) {
        crate::cfg::std! {
            if {
                ::std::eprintln!("Aborting due to drop resource panicked.");
                ::std::process::abort();
            } else {
                panic!("Aborting due to drop resource panicked.");
            }
        }
    }
}

// -----------------------------------------------------------------------------
// ResData

/// Raw storage for a single resource instance.
///
/// Manages memory allocation, initialization state, and change tracking ticks.
pub struct ResData {
    name: DebugName,
    layout: Layout,
    dropper: Option<Dropper>,
    data: *mut u8,
    added: Tick,
    changed: Tick,
}

impl Debug for ResData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Res")
            .field("name", &self.name)
            .field("is_active", &self.is_active())
            .finish()
    }
}

impl Drop for ResData {
    fn drop(&mut self) {
        unsafe {
            self.clear();
        }
    }
}

impl ResData {
    /// Creates a new `ResData` from resource type information.
    ///
    /// # Safety
    /// - `info` must correctly describe the resource type
    /// - The memory layout must be valid for allocation
    pub(crate) unsafe fn new(info: &ResourceInfo) -> Self {
        let name = info.debug_name();
        let dropper = info.dropper();
        let layout = info.layout();

        Self {
            name,
            layout,
            dropper,
            data: ptr::null_mut(),
            added: Tick::new(0),
            changed: Tick::new(0),
        }
    }

    /// Returns whether the resource is currently initialized.
    #[inline(always)]
    pub fn is_active(&self) -> bool {
        !self.data.is_null()
    }

    /// Returns the debug name of the resource type.
    #[inline(always)]
    pub fn debug_name(&self) -> DebugName {
        self.name
    }

    /// Returns a pointer to the resource data if initialized.
    #[inline]
    pub fn get_data(&self) -> Option<Ptr<'_>> {
        unsafe { Some(Ptr::new(NonNull::new(self.data)?)) }
    }

    /// Returns a pointer to the resource data if initialized.
    #[inline]
    pub fn get_data_mut(&mut self) -> Option<PtrMut<'_>> {
        unsafe { Some(PtrMut::new(NonNull::new(self.data)?)) }
    }

    /// Returns the added tick if the resource is initialized.
    #[inline]
    pub fn get_added(&self) -> Option<Tick> {
        if self.is_active() {
            Some(self.added)
        } else {
            None
        }
    }

    /// Returns the changed tick if the resource is initialized.
    #[inline]
    pub fn get_changed(&self) -> Option<Tick> {
        if self.is_active() {
            Some(self.changed)
        } else {
            None
        }
    }

    /// Drops the resource data if initialized.
    ///
    /// # Safety
    /// - If the data is NonSend, the function must be call in correct thread.
    pub unsafe fn clear(&mut self) {
        if let Some(data) = NonNull::new(self.data) {
            let guard = AbortOnDropFail;
            unsafe {
                if let Some(dropper) = self.dropper {
                    dropper.call(OwningPtr::new(data));
                }
                if self.layout.size() != 0 {
                    malloc::dealloc(self.data, self.layout);
                }
                self.data = ptr::null_mut();
            }
            ::core::mem::forget(guard);
        }
    }

    /// # Safety
    /// - If the data is NonSend, the function must be call in correct thread.
    pub unsafe fn leak(&mut self) -> Option<(NonNull<u8>, Tick, Tick)> {
        if !self.data.is_null() {
            let ptr = self.data;
            self.data = ptr::null_mut();
            NonNull::new(ptr).map(|ptr| (ptr, self.added, self.changed))
        } else {
            None
        }
    }

    /// # Safety
    /// - `ptr` must matche the resource's layout
    /// - If the data is NonSend, the function must be call in correct thread.
    pub unsafe fn from_raw(&mut self, ptr: NonNull<u8>, added: Tick, changed: Tick) {
        unsafe {
            self.clear();
        }
        self.added = added;
        self.changed = changed;
        self.data = ptr.as_ptr();
    }

    /// Drop the resource in situ.
    ///
    /// # Safety
    /// - `T` must matche the resource's layout
    /// - If the data is NonSend, the function must be call in correct thread.
    pub unsafe fn drop_in_place<T: Resource>(&mut self) {
        if !self.data.is_null() {
            unsafe {
                self.data.cast::<T>().drop_in_place();
            }
            if self.layout.size() != 0 {
                unsafe { malloc::dealloc(self.data, self.layout) };
            }
            self.data = ptr::null_mut();
        }
    }

    /// Removes the resource and returns ownership of its data.
    ///
    /// # Safety
    /// - `T` must matche the resource's layout
    /// - If the data is NonSend, the function must be call in correct thread.
    pub unsafe fn remove<T: Resource>(&mut self) -> Option<T> {
        if self.data.is_null() {
            return None;
        }

        let ret = unsafe { ptr::read::<T>(self.data as *mut T) };

        if self.layout.size() != 0 {
            unsafe { malloc::dealloc(self.data, self.layout) };
        }
        self.data = ptr::null_mut();

        Some(ret)
    }

    /// Inserts a new resource value.
    ///
    /// # Safety
    /// - `value` must matche the resource's layout
    /// - `tick` must be a valid system tick
    /// - If the data is NonSend, the function must be call in correct thread.
    pub unsafe fn insert<T: Resource>(&mut self, value: T, tick: Tick) {
        debug_assert_eq!(Layout::new::<T>(), self.layout);
        voker_ptr::into_owning!(value);
        unsafe { self.insert_untyped(value, tick) };
    }

    /// Inserts a new resource value.
    ///
    /// # Safety
    /// - `value` must point to valid data matching the resource's layout
    /// - `tick` must be a valid system tick
    /// - If the data is NonSend, the function must be call in correct thread.
    pub unsafe fn insert_untyped(&mut self, value: OwningPtr<'_>, tick: Tick) {
        if let Some(data) = NonNull::new(self.data) {
            if let Some(dropper) = self.dropper {
                let guard = AbortOnDropFail;
                unsafe {
                    dropper.call(OwningPtr::new(data));
                }
                ::core::mem::forget(guard);
            }
        } else {
            let layout = self.layout;
            if layout.size() == 0 {
                let align = NonZeroUsize::new(layout.align()).unwrap();
                self.data = NonNull::without_provenance(align).as_ptr();
            } else {
                self.data = NonNull::new(unsafe { malloc::alloc(layout) })
                    .unwrap_or_else(|| malloc::handle_alloc_error(layout))
                    .as_ptr();
            };
            self.added = tick;
        }
        unsafe {
            self.changed = tick;
            ptr::copy_nonoverlapping::<u8>(value.as_ptr(), self.data, self.layout.size());
        }
    }

    /// Returns an untyped reference to the resource if initialized.
    #[inline]
    pub fn get_ref(&self, last_run: Tick, this_run: Tick) -> Option<UntypedRef<'_>> {
        let data = NonNull::new(self.data)?;
        Some(UntypedRef {
            value: unsafe { Ptr::new(data) },
            ticks: TicksRef {
                added: &self.added,
                changed: &self.changed,
                last_run,
                this_run,
            },
        })
    }

    /// Returns an untyped mutable reference to the resource if initialized.
    #[inline]
    pub fn get_mut(&mut self, last_run: Tick, this_run: Tick) -> Option<UntypedMut<'_>> {
        let data = NonNull::new(self.data)?;
        Some(UntypedMut {
            value: unsafe { PtrMut::new(data) },
            ticks: TicksMut {
                added: &mut self.added,
                changed: &mut self.changed,
                last_run,
                this_run,
            },
        })
    }

    /// Updates ticks with quick-check logic.
    #[inline(always)]
    pub(super) fn quick_check(&mut self, now: Tick, fall_back: Tick) {
        self.added.quick_check(now, fall_back);
        self.changed.quick_check(now, fall_back);
    }
}
