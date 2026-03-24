use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::ptr::NonNull;

use crate::world::World;

/// A copyable raw handle to [`World`] with manually enforced borrow rules.
///
/// `UnsafeWorld` is used in performance-sensitive internals where temporarily
/// splitting access patterns is necessary (for example: read-only world access
/// plus mutable access to cached state). It behaves like an unchecked pointer:
/// the compiler can no longer enforce aliasing and thread-safety rules for you.
///
/// The exposed methods are `unsafe` because the caller must uphold the borrow
/// invariants required by Rust and by ECS world semantics.
#[derive(Clone, Copy)]
pub struct UnsafeWorld<'a> {
    world: NonNull<World>,
    _marker: PhantomData<&'a UnsafeCell<World>>,
}

unsafe impl Send for UnsafeWorld<'_> {}
unsafe impl Sync for UnsafeWorld<'_> {}

impl<'a> From<&'a World> for UnsafeWorld<'a> {
    /// Creates an [`UnsafeWorld`] from a shared world reference.
    fn from(value: &'a World) -> Self {
        UnsafeWorld {
            world: NonNull::from_ref(value),
            _marker: PhantomData,
        }
    }
}

impl<'a> From<&'a mut World> for UnsafeWorld<'a> {
    /// Creates an [`UnsafeWorld`] from an exclusive world reference.
    fn from(value: &'a mut World) -> Self {
        UnsafeWorld {
            world: NonNull::from_mut(value),
            _marker: PhantomData,
        }
    }
}

impl World {
    /// Returns a raw-access handle to this world.
    ///
    /// This does not grant any additional guarantees by itself. Safety must be
    /// enforced by the code that later dereferences the returned handle.
    pub const fn unsafe_world(&self) -> UnsafeWorld<'_> {
        UnsafeWorld {
            world: NonNull::from_ref(self),
            _marker: PhantomData,
        }
    }
}

impl<'a> UnsafeWorld<'a> {
    /// Reinterprets this handle as a shared world reference.
    ///
    /// # Safety
    /// - Access must remain read-only for the duration of the borrow.
    /// - The caller must ensure no concurrent mutable access that would violate
    ///   Rust aliasing rules.
    #[inline(always)]
    pub const unsafe fn read_only(self) -> &'a World {
        unsafe { &*self.world.as_ptr() }
    }

    /// Reinterprets this handle as a mutable world reference for data mutation.
    ///
    /// This is intended for operations that mutate existing world data without
    /// changing world structure.
    ///
    /// # Safety
    /// - The caller must ensure exclusive mutable access according to Rust
    ///   aliasing rules.
    /// - Only data-level mutation is allowed:
    ///   - do not add/remove entities or resources,
    ///   - do not register new types,
    ///   - do not allocate new ids.
    #[inline(always)]
    pub const unsafe fn data_mut(self) -> &'a mut World {
        unsafe { &mut *self.world.as_ptr() }
    }

    /// Reinterprets this handle as a fully mutable world reference.
    ///
    /// # Safety
    /// - There must be no other active borrows (shared or mutable) that alias
    ///   this world for the returned lifetime.
    #[inline(always)]
    pub const unsafe fn full_mut(self) -> &'a mut World {
        unsafe { &mut *self.world.as_ptr() }
    }

    /// Returns the underlying non-null world pointer.
    ///
    /// This does not dereference the pointer.
    #[inline(always)]
    pub const fn into_inner(self) -> NonNull<World> {
        self.world
    }
}
