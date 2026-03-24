#![expect(unsafe_code, reason = "Cast pointers to references is unsafe.")]

use core::any::TypeId;
use voker_ptr::{Ptr, PtrMut};

use crate::Reflect;
use crate::info::{TypePath, Typed};
use crate::registry::FromType;

#[derive(Clone)]
pub struct ReflectFromPtr {
    type_id: TypeId,
    from_ptr: unsafe fn(Ptr) -> &dyn Reflect,
    from_ptr_mut: unsafe fn(PtrMut) -> &mut dyn Reflect,
}

impl ReflectFromPtr {
    /// Returns the [`TypeId`] that the [`ReflectFromPtr`] was constructed for.
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Convert `Ptr` into `&dyn Reflect`.
    ///
    /// # Safety
    ///
    /// `val` must be a pointer to value of the type that the [`ReflectFromPtr`] was constructed for.
    /// This can be verified by checking that the type id returned by [`ReflectFromPtr::type_id`] is the expected one.
    pub unsafe fn as_reflect<'a>(&self, val: Ptr<'a>) -> &'a dyn Reflect {
        // SAFETY: contract uphold by the caller.
        unsafe { (self.from_ptr)(val) }
    }

    /// Convert `PtrMut` into `&mut dyn Reflect`.
    ///
    /// # Safety
    ///
    /// `val` must be a pointer to a value of the type that the [`ReflectFromPtr`] was constructed for
    /// This can be verified by checking that the type id returned by [`ReflectFromPtr::type_id`] is the expected one.
    pub unsafe fn as_reflect_mut<'a>(&self, val: PtrMut<'a>) -> &'a mut dyn Reflect {
        // SAFETY: contract uphold by the caller.
        unsafe { (self.from_ptr_mut)(val) }
    }

    /// Get a function pointer to turn a `Ptr` into `&dyn Reflect` for
    /// the type this [`ReflectFromPtr`] was constructed for.
    ///
    /// # Safety
    ///
    /// When calling the unsafe function returned by this method you must ensure that:
    /// - The input `Ptr` points to the `Reflect` type this `ReflectFromPtr`
    ///   was constructed for.
    pub fn from_ptr(&self) -> unsafe fn(Ptr) -> &dyn Reflect {
        self.from_ptr
    }

    /// Get a function pointer to turn a `PtrMut` into `&mut dyn Reflect` for
    /// the type this [`ReflectFromPtr`] was constructed for.
    ///
    /// # Safety
    ///
    /// When calling the unsafe function returned by this method you must ensure that:
    /// - The input `PtrMut` points to the `Reflect` type this `ReflectFromPtr`
    ///   was constructed for.
    pub fn from_ptr_mut(&self) -> unsafe fn(PtrMut) -> &mut dyn Reflect {
        self.from_ptr_mut
    }
}

impl<T: Typed + Reflect> FromType<T> for ReflectFromPtr {
    fn from_type() -> Self {
        ReflectFromPtr {
            type_id: TypeId::of::<T>(),
            from_ptr: |ptr| {
                // SAFETY: `from_ptr_mut` is either called in `ReflectFromPtr::as_reflect`
                // or returned by `ReflectFromPtr::from_ptr`, both lay out the invariants
                // required by `deref`
                ptr.debug_assert_aligned::<T>();
                unsafe { ptr.consume::<T>() as &dyn Reflect }
            },
            from_ptr_mut: |ptr| {
                // SAFETY: same as above
                ptr.debug_assert_aligned::<T>();
                unsafe { ptr.consume::<T>() as &mut dyn Reflect }
            },
        }
    }
}

// Explicitly implemented here so that code readers do not need
// to ponder the principles of proc-macros in advance.
impl TypePath for ReflectFromPtr {
    #[inline(always)]
    fn type_path() -> &'static str {
        "voker_reflect::registry::ReflectFromPtr"
    }

    #[inline(always)]
    fn type_name() -> &'static str {
        "ReflectFromPtr"
    }

    #[inline(always)]
    fn type_ident() -> &'static str {
        "ReflectFromPtr"
    }

    #[inline(always)]
    fn module_path() -> Option<&'static str> {
        Some("voker_reflect::registry")
    }
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use super::ReflectFromPtr;
    use crate::info::TypePath;

    #[test]
    fn type_path() {
        assert!(ReflectFromPtr::type_path() == "voker_reflect::registry::ReflectFromPtr");
        assert!(ReflectFromPtr::module_path() == Some("voker_reflect::registry"));
        assert!(ReflectFromPtr::type_ident() == "ReflectFromPtr");
        assert!(ReflectFromPtr::type_name() == "ReflectFromPtr");
    }
}
