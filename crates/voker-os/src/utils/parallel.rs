//! Per-thread local aggregation helper.
//!
//! `Parallel<T>` stores one local `T` per thread (or a single local value in
//! fallback mode) and provides convenience methods to mutate and drain them.

use alloc::vec::Vec;
use core::cell::RefCell;
use core::ops::DerefMut;

use crate::cfg;

cfg::std! {
    if {
        use thread_local::ThreadLocal;

        /// A cohesive set of thread-local values of a given type.
        ///
        /// Mutable references can be fetched if `T: Default` via [`Parallel::scope`]
        /// or [`Parallel::local_mut`].
        ///
        /// Each thread owns an independent local value.
        pub struct Parallel<T: Send> {
            locals: ThreadLocal<RefCell<T>>,
        }
    } else {
        use core::cell::UnsafeCell;

        /// A cohesive set of thread-local values of a given type.
        ///
        /// Mutable references can be fetched if `T: Default` via [`Parallel::scope`]
        /// or [`Parallel::local_mut`].
        ///
        /// Fallback implementation; can only be used in a single thread.
        pub struct Parallel<T: Send> {
            locals: UnsafeCell<Option<RefCell<T>>>,
        }

        #[expect(unsafe_code, reason = "fallback implementation")]
        unsafe impl<T: Send> Sync for Parallel<T> {}

        impl<T: Send> Parallel<T> {
            #[inline(always)]
            const fn inner(&self) -> &Option<RefCell<T>> {
                #[expect(unsafe_code, reason = "fallback implementation")]
                unsafe{ &*self.locals.get() }
            }

            #[inline(always)]
            const fn inner_mut(&mut self) -> &mut Option<RefCell<T>> {
                self.locals.get_mut()
            }
        }
    }
}

// `Default` is manually implemented to avoid the `T: Default` bound.
impl<T: Send> Default for Parallel<T> {
    fn default() -> Self {
        cfg::std! {
            if {
                Self { locals: Default::default() }
            } else {
                Self { locals: UnsafeCell::new(None) }
            }
        }
    }
}

impl<T: Send> Parallel<T> {
    /// Clears all of the stored thread local values.
    ///
    /// After this call, existing per-thread values are removed. Accessing
    /// locals again through [`Parallel::scope`] or [`Parallel::local_mut`]
    /// will recreate values from `Default`.
    pub fn clear(&mut self) {
        cfg::std! {
            if {
                self.locals.clear();
            } else {
                self.inner_mut().take();
            }
        }
    }

    /// Gets a mutable iterator over all of the per-thread queues.
    ///
    /// The iterator yields currently materialized local values. The ordering
    /// is not guaranteed.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &'_ mut T> {
        cfg::std! {
            if {
                self.locals.iter_mut().map(RefCell::get_mut)
            } else {
                self.inner_mut().iter_mut().map(RefCell::get_mut)
            }
        }
    }
}

impl<T: Default + Send> Parallel<T> {
    /// Retrieves the thread-local value for the current thread and runs `f` on it.
    ///
    /// If there is no thread-local value, it will be initialized to its default.
    ///
    /// # Panics
    ///
    /// Panics if the same thread tries to mutably borrow the local value
    /// reentrantly (through `RefCell` borrow rules).
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_os::utils::Parallel;
    ///
    /// let p = Parallel::<usize>::default();
    /// p.scope(|v| *v += 1);
    /// assert_eq!(p.scope(|v| *v), 1);
    /// ```
    pub fn scope<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        f(&mut *self.local_mut())
    }

    /// Mutably borrows the thread-local value.
    ///
    /// If there is no thread-local value, it will be initialized to its default.
    ///
    /// # Panics
    ///
    /// Panics if the same thread already holds a mutable borrow of the local
    /// value.
    pub fn local_mut(&self) -> impl DerefMut<Target = T> + '_ {
        cfg::std! {
            if {
                self.locals.get_or(|| RefCell::new(Default::default())).borrow_mut()
            } else {
                #[expect(unsafe_code, reason = "fallback implementation")]
                let inner = unsafe{ &mut *self.locals.get() };

                if inner.is_none() {
                    *inner = Some(RefCell::new(Default::default()));
                }

                inner.as_mut().unwrap().borrow_mut()
            }
        }
    }
}

impl<T, I> Parallel<I>
where
    I: IntoIterator<Item = T> + Default + Send + 'static,
{
    /// Drains all enqueued items from all threads and returns an iterator over them.
    ///
    /// Unlike [`Vec::drain`], this will piecemeal remove chunks of the data stored.
    /// If iteration is terminated part way, the rest of the enqueued items in the same
    /// chunk will be dropped, and the rest of the undrained elements will remain.
    ///
    /// The ordering is not guaranteed.
    ///
    /// Each thread-local container is replaced with `I::default()` before
    /// yielding items.
    pub fn drain(&mut self) -> impl Iterator<Item = T> + '_ {
        cfg::std! {
            if {
                self.locals.iter_mut().flat_map(|item| item.take())
            } else {
                self.inner().iter().flat_map(|item| item.take())
            }
        }
    }
}

impl<T: Send> Parallel<Vec<T>> {
    /// Collects all enqueued items from all threads and appends them to `out`.
    ///
    /// The ordering is not guaranteed.
    ///
    /// This method reserves once based on total queued length and then appends
    /// each per-thread vector into `out`.
    pub fn drain_into(&mut self, out: &mut Vec<T>) {
        cfg::std! {
            if {
                let size = self
                    .locals
                    .iter_mut()
                    .map(|queue| queue.get_mut().len())
                    .sum();
                out.reserve(size);
                for queue in self.locals.iter_mut() {
                    out.append(queue.get_mut());
                }
            } else {
                if let Some(cell) = self.inner_mut() {
                    out.append(&mut cell.borrow_mut());
                }
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(all(test, feature = "std"))]
mod tests {
    use alloc::vec::Vec;
    use std::sync::Arc;
    use std::thread;

    use super::Parallel;

    #[test]
    fn scope_and_local_mut() {
        let p = Parallel::<usize>::default();
        assert_eq!(*p.local_mut(), 0);

        p.scope(|v| *v = 7);
        assert_eq!(p.scope(|v| *v), 7);
    }

    #[test]
    fn clear_recreates_default() {
        let mut p = Parallel::<usize>::default();
        p.scope(|v| *v = 11);
        p.clear();
        assert_eq!(p.scope(|v| *v), 0);
    }

    #[test]
    fn iter_mut_visits_thread_locals() {
        let p = Arc::new(Parallel::<Vec<usize>>::default());

        thread::scope(|s| {
            for i in 1..=3 {
                let p = Arc::clone(&p);
                s.spawn(move || {
                    p.scope(|local| local.push(i));
                });
            }
            p.scope(|local| local.push(0));
        });

        let mut p = Arc::try_unwrap(p).ok().expect("single owner after scope");
        let mut seen = Vec::new();
        for local in p.iter_mut() {
            seen.extend(local.iter().copied());
        }
        seen.sort_unstable();
        assert_eq!(seen, [0, 1, 2, 3]);
    }

    #[test]
    fn drain_and_drain_into() {
        let p = Arc::new(Parallel::<Vec<usize>>::default());

        thread::scope(|s| {
            for i in 0..4 {
                let p = Arc::clone(&p);
                s.spawn(move || {
                    p.scope(|local| {
                        local.push(i);
                        local.push(i + 10);
                    });
                });
            }
        });

        let mut p = Arc::try_unwrap(p).ok().expect("single owner after scope");

        let mut drained: Vec<_> = p.drain().collect();
        drained.sort_unstable();
        assert_eq!(drained, [0, 1, 2, 3, 10, 11, 12, 13]);

        let p = Arc::new(Parallel::<Vec<usize>>::default());
        thread::scope(|s| {
            for i in 0..2 {
                let p = Arc::clone(&p);
                s.spawn(move || {
                    p.scope(|local| local.push(i));
                });
            }
        });

        let mut p = Arc::try_unwrap(p).ok().expect("single owner after scope");
        let mut out = Vec::from([99]);
        p.drain_into(&mut out);
        out.sort_unstable();
        assert_eq!(out, [0, 1, 99]);
    }
}
