use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::ptr::{self, NonNull};
use core::slice;

// -----------------------------------------------------------------------------
// ThinSlice and ThinSliceMut

/// A thin reference to a slice that stores only the pointer (no length).
///
/// This type is useful when the slice length is known from context and storing
/// it separately would waste memory. It provides shared access to the elements.
///
/// # Examples
///
/// ```
/// # use voker_ptr::ThinSlice;
/// let data = [1, 2, 3, 4, 5];
/// let thin = ThinSlice::from_ref(&data);
///
/// // The length must be provided when accessing
/// unsafe {
///     assert_eq!(thin.as_slice(5), &[1, 2, 3, 4, 5]);
///     assert_eq!(thin.get(2), &3);
/// }
/// ```
#[derive(Debug)]
#[repr(transparent)]
pub struct ThinSlice<'a, T> {
    _marker: PhantomData<&'a [T]>,
    ptr: NonNull<T>,
}

/// A thin mutable reference to a slice that stores only the pointer (no length).
///
/// This type is useful when the slice length is known from context and storing
/// it separately would waste memory. It provides exclusive access to the elements.
///
/// # Examples
///
/// ```
/// # use voker_ptr::ThinSliceMut;
/// let mut data = [1, 2, 3, 4, 5];
/// let thin = ThinSliceMut::from_mut(&mut data);
///
/// unsafe {
///     // Read and write elements
///     assert_eq!(thin.read(0), 1);
///     thin.write(0, 10);
///     assert_eq!(thin.get(0), &10);
///     
///     // Get as a slice
///     assert_eq!(thin.as_slice(5), &[10, 2, 3, 4, 5]);
/// }
/// ```
#[derive(Debug)]
#[repr(transparent)]
pub struct ThinSliceMut<'a, T> {
    _marker: PhantomData<&'a mut [T]>,
    ptr: NonNull<T>,
}

impl<T> Copy for ThinSlice<'_, T> {}
impl<T> Clone for ThinSlice<'_, T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

// -----------------------------------------------------------------------------
// From

impl<'a, T> From<&'a [T]> for ThinSlice<'a, T> {
    #[inline]
    fn from(slice: &'a [T]) -> Self {
        Self::from_ref(slice)
    }
}

impl<'a, T> From<&'a mut [T]> for ThinSlice<'a, T> {
    #[inline]
    fn from(slice: &'a mut [T]) -> Self {
        Self::from_mut(slice)
    }
}

impl<'a, T> From<&'a mut [T]> for ThinSliceMut<'a, T> {
    #[inline]
    fn from(slice: &'a mut [T]) -> Self {
        Self::from_mut(slice)
    }
}

impl<'a, T> From<&'a [UnsafeCell<T>]> for ThinSliceMut<'a, T> {
    #[inline]
    fn from(slice: &'a [UnsafeCell<T>]) -> Self {
        unsafe { Self::from_raw(NonNull::new_unchecked(slice.as_ptr() as *mut T)) }
    }
}

impl<'a, T> From<ThinSliceMut<'a, T>> for ThinSlice<'a, T> {
    #[inline(always)]
    fn from(value: ThinSliceMut<'a, T>) -> Self {
        Self {
            _marker: PhantomData,
            ptr: value.ptr,
        }
    }
}

impl<'a, T> From<ThinSlice<'a, UnsafeCell<T>>> for ThinSliceMut<'a, T> {
    #[inline(always)]
    fn from(value: ThinSlice<'a, UnsafeCell<T>>) -> Self {
        Self {
            _marker: PhantomData,
            ptr: value.ptr.cast(),
        }
    }
}

// -----------------------------------------------------------------------------
// Methods

impl<'a, T> ThinSlice<'a, T> {
    /// Return to the underlying pointer
    #[inline(always)]
    pub const fn into_inner(self) -> NonNull<T> {
        self.ptr
    }

    /// Creates a `ThinSlice` from a shared slice reference.
    #[inline(always)]
    pub const fn from_ref(r: &'a [T]) -> Self {
        Self {
            _marker: PhantomData,
            ptr: NonNull::from_ref(r).cast(),
        }
    }

    /// Creates a `ThinSlice` from a mutable slice reference.
    #[inline(always)]
    pub const fn from_mut(r: &'a mut [T]) -> Self {
        Self {
            _marker: PhantomData,
            ptr: NonNull::from_ref(r).cast(),
        }
    }

    /// Creates a `ThinSlice` from a raw pointer.
    ///
    /// # Safety
    /// - The pointer must be valid for reads for the lifetime `'a`
    /// - The caller must ensure proper bounds when accessing elements
    #[inline(always)]
    pub const unsafe fn from_raw(ptr: NonNull<T>) -> Self {
        Self {
            _marker: PhantomData,
            ptr,
        }
    }

    /// Returns a shared reference to the element at `index`.
    ///
    /// # Safety
    /// - `index` must be within bounds
    /// - The element must be properly initialized
    #[inline(always)]
    pub const unsafe fn get(self, index: usize) -> &'a T {
        unsafe { &*self.ptr.as_ptr().add(index) }
    }

    /// Returns a shared slice with the given length.
    ///
    /// # Safety
    /// - All elements in `0..len` must be properly initialized
    /// - `len` must not exceed the actual allocation size
    #[inline(always)]
    pub const unsafe fn as_slice(self, len: usize) -> &'a [T] {
        unsafe { slice::from_raw_parts(self.ptr.as_ptr(), len) }
    }

    /// Consume itself and return a slice with the same lifecycle
    ///
    /// Because `ThinSlice` is copyable, this function is the same as
    /// [`ThinSlice::as_slice`].
    ///
    /// # Safety
    /// - All elements in `0..len` must be properly initialized
    /// - `len` must not exceed the actual allocation size
    #[inline(always)]
    pub const unsafe fn consume(self, len: usize) -> &'a [T] {
        unsafe { slice::from_raw_parts(self.ptr.as_ptr(), len) }
    }

    /// Reads a copy of the element at `index`.
    ///
    /// # Safety
    /// - `index` must be within bounds
    /// - The element must be properly initialized
    #[inline(always)]
    pub const unsafe fn read(self, index: usize) -> T
    where
        T: Copy,
    {
        unsafe { ptr::read(self.ptr.as_ptr().add(index)) }
    }
}

impl<'a, T> ThinSliceMut<'a, T> {
    /// Return to the underlying pointer
    #[inline(always)]
    pub const fn into_inner(self) -> NonNull<T> {
        self.ptr
    }

    /// Reborrow this pointer with a shorter lifetime.
    ///
    /// This is useful when a helper function needs temporary mutable access
    /// without consuming the original `ThinSliceMut`.
    #[inline(always)]
    pub const fn reborrow(&mut self) -> ThinSliceMut<'_, T> {
        ThinSliceMut {
            _marker: PhantomData,
            ptr: self.ptr,
        }
    }

    /// Consume itself and return a slice with the same lifecycle
    ///
    /// # Safety
    /// - All elements in `0..len` must be properly initialized
    /// - `len` must not exceed the actual allocation size
    #[inline(always)]
    pub const unsafe fn consume(self, len: usize) -> &'a mut [T] {
        unsafe { slice::from_raw_parts_mut(self.ptr.as_ptr(), len) }
    }

    /// Creates a `ThinSliceMut` from a mutable slice reference.
    #[inline(always)]
    pub const fn from_mut(r: &'a mut [T]) -> Self {
        Self {
            _marker: PhantomData,
            ptr: NonNull::from_ref(r).cast(),
        }
    }

    /// Creates a `ThinSliceMut` from a raw pointer.
    ///
    /// # Safety
    /// - The pointer must be valid for reads and writes for the lifetime `'a`
    /// - No other references to the same memory must exist
    /// - The caller must ensure proper bounds when accessing elements
    #[inline(always)]
    pub const unsafe fn from_raw(ptr: NonNull<T>) -> Self {
        Self {
            _marker: PhantomData,
            ptr,
        }
    }

    /// Returns a shared reference to the element at `index`.
    ///
    /// # Safety
    /// - `index` must be within bounds
    /// - The element must be properly initialized
    #[inline(always)]
    pub const unsafe fn get(&self, index: usize) -> &T {
        unsafe { &*self.ptr.as_ptr().add(index) }
    }

    /// Returns a mutable reference to the element at `index`.
    ///
    /// # Safety
    /// - `index` must be within bounds
    /// - The element must be properly initialized
    #[inline(always)]
    pub const unsafe fn get_mut(&mut self, index: usize) -> &mut T {
        unsafe { &mut *self.ptr.as_ptr().add(index) }
    }

    /// Returns a shared slice with the given length.
    ///
    /// # Safety
    /// - All elements in `0..len` must be properly initialized
    /// - `len` must not exceed the actual allocation size
    #[inline(always)]
    pub const unsafe fn as_slice(&self, len: usize) -> &[T] {
        unsafe { slice::from_raw_parts(self.ptr.as_ptr(), len) }
    }

    /// Returns a mutable slice with the given length.
    ///
    /// # Safety
    /// - All elements in `0..len` must be properly initialized
    /// - `len` must not exceed the actual allocation size
    #[inline(always)]
    pub const unsafe fn as_slice_mut(&mut self, len: usize) -> &mut [T] {
        unsafe { slice::from_raw_parts_mut(self.ptr.as_ptr(), len) }
    }

    /// Reads a copy of the element at `index`.
    ///
    /// # Safety
    /// - `index` must be within bounds
    /// - The element must be properly initialized
    #[inline(always)]
    pub const unsafe fn read(&self, index: usize) -> T
    where
        T: Copy,
    {
        unsafe { ptr::read(self.ptr.as_ptr().add(index)) }
    }

    /// Writes a copy of the value to the element at `index`.
    ///
    /// # Safety
    /// - `index` must be within bounds
    /// - The slot at `index` must be properly initialized (for `Copy` types this is optional)
    #[inline(always)]
    pub const unsafe fn write(&self, index: usize, value: T)
    where
        T: Copy,
    {
        unsafe { ptr::write(self.ptr.as_ptr().add(index), value) }
    }
}

#[cfg(test)]
mod tests {
    use super::{ThinSlice, ThinSliceMut};

    #[test]
    fn slice_read() {
        let data = [1_i32, 2, 3];
        let thin = ThinSlice::from_ref(&data);

        let second = unsafe { thin.read(1) };
        assert_eq!(second, 2);

        let full = unsafe { thin.as_slice(3) };
        assert_eq!(full, &[1, 2, 3]);
    }

    #[test]
    fn slice_mut_write() {
        let mut data = [4_i32, 5, 6];
        let mut thin = ThinSliceMut::from_mut(&mut data);

        let mut sub = thin.reborrow();
        let slot = unsafe { sub.get_mut(2) };
        *slot = 42;

        let full = unsafe { thin.as_slice(3) };
        assert_eq!(full, &[4, 5, 42]);
    }

    #[test]
    fn slice_mut_consume() {
        let mut data = [9_i32, 8, 7];
        let thin = ThinSliceMut::from_mut(&mut data);
        let out = unsafe { thin.consume(3) };
        out[0] = 11;

        assert_eq!(out, &[11, 8, 7]);
    }
}
