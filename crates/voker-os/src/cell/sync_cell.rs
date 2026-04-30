//! A reimplementation of the currently unstable [`core::sync::Exclusive`]
#![expect(unsafe_code, reason = "SyncCell requires unsafe code.")]

use core::pin::Pin;

/// See [`core::sync::Exclusive`]
///
/// # Example
///
/// ```
/// # use core::cell::Cell;
/// # use voker_os::cell::SyncCell;
///
/// async fn other() {}
///
/// fn assert_sync<T: Sync>(t: T) {}
///
/// struct State<F> {
///     future: SyncCell<F>
/// }
///
/// assert_sync(State {
///     future: SyncCell::new(async {
///         // including Cell, but SyncCell is `sync`
///         let cell = Cell::new(1);
///         let cell_ref = &cell;
///         let val = cell_ref.get();
///     })
/// });
/// ```
#[repr(transparent)]
pub struct SyncCell<T: ?Sized> {
    inner: T,
}

// SAFETY: `Sync` only allows multithreaded access via immutable reference.
//
// As `SyncCell` requires an exclusive reference to access the wrapped value for `!Sync` types,
// marking this type as `Sync` does not actually allow unsynchronized access to the inner value.
unsafe impl<T: ?Sized> Sync for SyncCell<T> {}

impl<T: Sized> SyncCell<T> {
    /// Wrap a value in an `SyncCell`.
    #[must_use]
    #[inline(always)]
    pub const fn new(inner: T) -> Self {
        Self { inner }
    }

    /// Unwrap the value contained in the `SyncCell`.
    #[must_use]
    #[inline(always)]
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T: ?Sized> SyncCell<T> {
    /// Gets exclusive access to the underlying value.
    #[must_use]
    #[inline]
    pub const fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Gets pinned exclusive access to the underlying value.
    #[must_use]
    #[inline]
    pub const fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut T> {
        // SAFETY: `SyncCell` can only produce `&mut T` if itself is unpinned
        // `Pin::map_unchecked_mut` is not const, so we do this conversion manually
        unsafe { Pin::new_unchecked(&mut self.get_unchecked_mut().inner) }
    }

    /// Build a _mutable_ reference to an `SyncCell<T>` from
    /// a _mutable_ reference to a `T`. This allows you to skip
    /// building an `SyncCell` with [`SyncCell::new`].
    #[must_use]
    #[inline]
    pub const fn from_mut(r: &'_ mut T) -> &'_ mut SyncCell<T> {
        // SAFETY: repr is ≥ C, so refs have the same layout; and `SyncCell` properties are `&mut`-agnostic
        unsafe { &mut *(r as *mut T as *mut SyncCell<T>) }
    }

    /// Build a _pinned mutable_ reference to an `SyncCell<T>` from
    /// a _pinned mutable_ reference to a `T`. This allows you to skip
    /// building an `SyncCell` with [`SyncCell::new`].
    #[must_use]
    #[inline]
    pub const fn from_pin_mut(r: Pin<&'_ mut T>) -> Pin<&'_ mut SyncCell<T>> {
        // SAFETY: `SyncCell` can only produce `&mut T` if itself is unpinned
        // `Pin::map_unchecked_mut` is not const, so we do this conversion manually
        unsafe { Pin::new_unchecked(Self::from_mut(r.get_unchecked_mut())) }
    }
}

impl<T> From<T> for SyncCell<T> {
    #[inline]
    fn from(t: T) -> Self {
        Self::new(t)
    }
}

impl<T> AsRef<T> for SyncCell<T>
where
    T: Sync + ?Sized,
{
    #[inline]
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

impl<T> Clone for SyncCell<T>
where
    T: Sync + Clone,
{
    #[inline]
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Copy for SyncCell<T> where T: Sync + Copy {}
