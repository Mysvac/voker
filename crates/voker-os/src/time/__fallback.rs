//! Fallback implementations of `Instant` and `SystemTime` for `no_std` environments.
//!
//! These implementations are suitable when the standard library's time facilities are unavailable.
//!
//! ## Instant
//!
//! **Supported Architectures (Built-in):**
//! - `x86`, `x86_64`: Uses `rdtsc` instruction (Time Stamp Counter)
//! - `aarch64`: Uses `cntvoker_el0` register (Virtual Counter)
//!
//! ```rust, ignore
//! // Default behavior on supported architectures
//! let start = Instant::now();
//! // ... perform work ...
//! let elapsed = start.elapsed();
//! ```
//!
//! **Unsupported Architectures:**
//! You must provide a custom timer function before using `Instant`:
//!
//! ```rust, ignore
//! // Provide a monotonic nanosecond counter
//! unsafe {
//!     Instant::set_elapsed_getter(|| {
//!         // Read hardware timer and convert to nanoseconds
//!         Duration::from_nanos(read_hardware_counter())
//!     });
//! }
//! ```
//!
//! ## SystemTime
//!
//! Unlike `Instant`, **SystemTime has no default implementation** in `no_std` mode.
//! You must always configure it manually:
//!
//! ```rust, ignore
//! // Configure with a function returning time since Unix epoch
//! unsafe {
//!     SystemTime::set_elapsed_getter(|| {
//!         // Example: read from Real-Time Clock (RTC)
//!         let seconds_since_epoch = read_rtc_seconds();
//!         Duration::from_secs(seconds_since_epoch)
//!     });
//! }
//!
//! // Now you can use SystemTime
//! let now = SystemTime::now();
//! let timestamp = now.duration_since(SystemTime::UNIX_EPOCH).unwrap();
//! ```
//!
//! The [`SystemTime::UNIX_EPOCH`] will be [`Duration::ZERO`] in `no_std` mod.
//!
//! ## Note
//!
//! If the `set_elapsed_getter` is not set, it will panic when calling related functions.
//!
//! But if the time measurement functions is not used, it can be omitted.
#![expect(unsafe_code, reason = "Instant fallback requires unsafe")]

use core::fmt;
use core::time::Duration;
use core::ops::{Add, AddAssign, Sub, SubAssign};

use crate::sync::atomic::{AtomicPtr, Ordering};

// -----------------------------------------------------------------------------
// Elapsed Getter

/// In fallback mode, users need to manually provide a timer function for Instance.
///
/// Call [`Instant::set_elapsed_getter`] .
static INSTANT_ELAPSED_GETTER: AtomicPtr<()> = AtomicPtr::new(instant_unset_getter as *mut ());

/// In fallback mode, users need to manually provide a timer function for SystemTime.
///
/// Call [`SystemTime::set_elapsed_getter`] .
static SYSTEM_ELAPSED_GETTER: AtomicPtr<()> = AtomicPtr::new(system_time_unset_getter as *mut ());

/// Default timer
fn instant_unset_getter() -> Duration {
    crate::cfg::switch! {
        #[cfg(target_arch = "x86")] => {
            // SAFETY: standard technique for getting a nanosecond counter on x86
            let nanos = unsafe {
                core::arch::x86::_rdtsc()
            };
            return Duration::from_nanos(nanos);
        }
        #[cfg(target_arch = "x86_64")] => {
            // SAFETY: standard technique for getting a nanosecond counter on x86_64
            let nanos = unsafe {
                core::arch::x86_64::_rdtsc()
            };
            return Duration::from_nanos(nanos);
        }
        #[cfg(target_arch = "aarch64")] => {
            // SAFETY: standard technique for getting a nanosecond counter of aarch64
            let nanos = unsafe {
                let mut ticks: u64;
                core::arch::asm!("mrs {}, cntvoker_el0", out(reg) ticks);
                ticks
            };
            return Duration::from_nanos(nanos);
        }
        _ => {
            panic!("An elapsed time getter has not been provided to `Instant`. Please use `Instant::set_elapsed_getter(...)` before calling `Instant::now()`")
        }
    }
}

fn system_time_unset_getter() -> Duration {
    panic!(
        "An elapsed time getter has not been provided to `SystemTime`. Please use `SystemTime::set_elapsed_getter(...)` before calling `SystemTime::now()`"
    )
}

// -----------------------------------------------------------------------------
// Instant

/// A measurement of a monotonically nondecreasing clock. Opaque and useful only with [`Duration`].
///
/// Fallback implementation suitable for a `no_std` environment.
///
/// # Platform-specific behavior
///
/// If you are on any of the following target architectures, this is a drop-in replacement:
///
/// - `x86`
/// - `x86_64`
/// - `aarch64`
///
/// On any other architecture, you must call [`Instant::set_elapsed_getter`], providing a method
/// which when called supplies a monotonically increasing count of elapsed nanoseconds relative
/// to some arbitrary point in time.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant(Duration);

impl Instant {
    /// Returns an instant corresponding to "now".
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use voker_os::time::Instant;
    ///
    /// let now = Instant::now();
    /// ```
    #[must_use]
    pub fn now() -> Instant {
        let getter = INSTANT_ELAPSED_GETTER.load(Ordering::Acquire);

        // SAFETY: Function pointer is always valid
        let getter = unsafe { core::mem::transmute::<*mut (), fn() -> Duration>(getter) };

        Self((getter)())
    }

    /// Configures the timer function used by [`Instant::now`].
    ///
    /// This function must be called before using `Instant` on architectures
    /// without a default implementation.
    ///
    /// # Safety
    ///
    /// The provided function must satisfy these requirements:
    ///
    /// 1. **Monotonic**: Returned values must never decrease
    /// 2. **Steady**: The rate of increase should be consistent
    /// 3. **Accurate**: Should represent actual elapsed time
    /// 4. **Thread-safe**: Must be safe to call from multiple threads
    /// 5. **Valid pointer**: The function pointer must remain valid for the lifetime of the program
    pub unsafe fn set_elapsed_getter(getter: fn() -> Duration) {
        INSTANT_ELAPSED_GETTER.store(getter as *mut _, Ordering::Release);
    }

    /// Returns the amount of time elapsed from another instant to this one,
    /// or zero duration if that instant is later than this one.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use voker_os::time::{Duration, Instant};
    /// use voker_os::thread::sleep;
    ///
    /// let now = Instant::now();
    /// sleep(Duration::new(1, 0));
    /// let new_now = Instant::now();
    /// println!("{:?}", new_now.duration_since(now));
    /// println!("{:?}", now.duration_since(new_now)); // 0ns
    /// ```
    #[must_use]
    pub fn duration_since(&self, earlier: Instant) -> Duration {
        self.saturating_duration_since(earlier)
    }

    /// Returns the amount of time elapsed from another instant to this one,
    /// or None if that instant is later than this one.
    ///
    /// Due to monotonicity bugs, even under correct logical ordering of the passed `Instant`s,
    /// this method can return `None`.
    #[must_use]
    pub fn checked_duration_since(&self, earlier: Instant) -> Option<Duration> {
        self.0.checked_sub(earlier.0)
    }

    /// Returns the amount of time elapsed from another instant to this one,
    /// or zero duration if that instant is later than this one.
    #[must_use]
    pub fn saturating_duration_since(&self, earlier: Instant) -> Duration {
        self.0.saturating_sub(earlier.0)
    }

    /// Returns the amount of time elapsed since this instant.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        Instant::now().saturating_duration_since(*self)
    }

    /// Returns `Some(t)` where `t` is the time `self + duration` if `t` can be represented as
    /// `Instant` (which means it's inside the bounds of the underlying data structure), `None`
    /// otherwise.
    pub fn checked_add(&self, duration: Duration) -> Option<Instant> {
        self.0.checked_add(duration).map(Instant)
    }

    /// Returns `Some(t)` where `t` is the time `self - duration` if `t` can be represented as
    /// `Instant` (which means it's inside the bounds of the underlying data structure), `None`
    /// otherwise.
    pub fn checked_sub(&self, duration: Duration) -> Option<Instant> {
        self.0.checked_sub(duration).map(Instant)
    }
}

impl Add<Duration> for Instant {
    type Output = Instant;

    /// # Panics
    ///
    /// This function may panic if the resulting point in time cannot be represented by the
    /// underlying data structure. See [`Instant::checked_add`] for a version without panic.
    fn add(self, other: Duration) -> Instant {
        self.checked_add(other)
            .expect("overflow when adding duration to instant")
    }
}

impl AddAssign<Duration> for Instant {
    fn add_assign(&mut self, other: Duration) {
        *self = *self + other;
    }
}

impl Sub<Duration> for Instant {
    type Output = Instant;

    fn sub(self, other: Duration) -> Instant {
        self.checked_sub(other)
            .expect("overflow when subtracting duration from instant")
    }
}

impl SubAssign<Duration> for Instant {
    fn sub_assign(&mut self, other: Duration) {
        *self = *self - other;
    }
}

impl Sub<Instant> for Instant {
    type Output = Duration;

    /// Returns the amount of time elapsed from another instant to this one,
    /// or zero duration if that instant is later than this one.
    fn sub(self, other: Instant) -> Duration {
        self.duration_since(other)
    }
}

impl fmt::Debug for Instant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// -----------------------------------------------------------------------------
// SystemTime

/// A measurement of the system clock, useful for talking to external entities
/// like the file system or other processes.
///
/// Distinct from [`Instant`], `SystemTime` can move forwards and backwards as
/// the system clock is adjusted. It corresponds to the POSIX `time_t` and
/// represents seconds since the Unix epoch.
///
/// # Important Note
///
/// Unlike `Instant`, **there is no default implementation** for `SystemTime`.
/// You **must** call [`SystemTime::set_elapsed_getter`] before using any `SystemTime`
/// functionality.
///
/// In this fallback implementation, `UNIX_EPOCH` is represented as
/// [`Duration::ZERO`] in the internal time scale. The actual mapping to
/// real-world time depends on the timer function configured via
/// [`SystemTime::set_elapsed_getter`].
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SystemTime(Duration);

impl SystemTime {
    /// An anchor in time corresponding to 1970-01-01 00:00:00 UTC.
    ///
    /// This constant is defined to be the Unix epoch on all systems. Using
    /// [`duration_since`](Self::duration_since) on a `SystemTime` instance
    /// returns the time elapsed since this point.
    ///
    /// In this fallback implementation, `UNIX_EPOCH` is represented as
    /// [`Duration::ZERO`] in the internal time scale. The actual mapping to
    /// real-world time depends on the timer function configured via
    /// [`SystemTime::set_elapsed_getter`].
    pub const UNIX_EPOCH: SystemTime = SystemTime(Duration::ZERO);

    /// Returns the system time corresponding to "now".
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use voker_os::time::SystemTime;
    ///
    /// let sys_time = SystemTime::now();
    /// ```
    #[must_use]
    pub fn now() -> SystemTime {
        let getter = SYSTEM_ELAPSED_GETTER.load(Ordering::Acquire);

        // SAFETY: Function pointer is always valid
        let getter = unsafe { core::mem::transmute::<*mut (), fn() -> Duration>(getter) };

        Self((getter)())
    }

    /// Configures the timer function used by [`SystemTime::now`].
    ///
    /// This function **must be called** before using `SystemTime`. Unlike
    /// `Instant`, there is no default implementation for system time.
    ///
    /// # Safety
    ///
    /// The provided function must:
    ///
    /// 1. Return time elapsed since the Unix epoch (1970-01-01 00:00:00 UTC)
    /// 2. Be thread-safe and safe to call from multiple threads
    /// 3. Provide a valid function pointer that remains valid for the lifetime of the program
    pub unsafe fn set_elapsed_getter(getter: fn() -> Duration) {
        SYSTEM_ELAPSED_GETTER.store(getter as *mut (), Ordering::Release);
    }

    /// Returns the amount of time elapsed from an earlier point in time.
    ///
    /// This function may fail because measurements taken earlier are not
    /// guaranteed to always be before later measurements (due to anomalies such
    /// as the system clock being adjusted either forwards or backwards).
    /// [`SystemTime`] can be used to measure elapsed time without this risk of failure.
    ///
    /// If successful, <code>[Ok]\([Duration])</code> is returned where the duration represents
    /// the amount of time elapsed from the specified measurement to this one.
    ///
    /// Returns an [`Err`] if `earlier` is later than `self`, and the error
    /// contains how far from `self` the time is.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use voker_os::time::SystemTime;
    ///
    /// let sys_time = SystemTime::now();
    /// let new_sys_time = SystemTime::now();
    /// let difference = new_sys_time.duration_since(sys_time)
    ///     .expect("Clock may have gone backwards");
    /// println!("{difference:?}");
    /// ```
    pub fn duration_since(&self, earlier: SystemTime) -> Result<Duration, SystemTimeError> {
        self.0.checked_sub(earlier.0).ok_or(SystemTimeError(self.0))
    }

    /// Returns the difference from this system time to the
    /// current clock time.
    ///
    /// This function may fail as the underlying system clock is susceptible to
    /// drift and updates (e.g., the system clock could go backwards), so this
    /// function might not always succeed. If successful, <code>[Ok]\([Duration])</code> is
    /// returned where the duration represents the amount of time elapsed from
    /// this time measurement to the current time.
    ///
    /// To measure elapsed time reliably, use [`Instant`] instead.
    ///
    /// Returns an [`Err`] if `self` is later than the current system time, and
    /// the error contains how far from the current system time `self` is.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use voker_os::thread::sleep;
    /// use voker_os::time::{Duration, SystemTime};
    ///
    /// let sys_time = SystemTime::now();
    /// let one_sec = Duration::from_secs(1);
    /// sleep(one_sec);
    /// assert!(sys_time.elapsed().unwrap() >= one_sec);
    /// ```
    pub fn elapsed(&self) -> Result<Duration, SystemTimeError> {
        SystemTime::now().duration_since(*self)
    }

    /// Returns `Some(t)` where `t` is the time `self + duration` if `t` can be represented as
    /// `SystemTime` (which means it's inside the bounds of the underlying data structure), `None`
    /// otherwise.
    pub fn checked_add(&self, duration: Duration) -> Option<SystemTime> {
        self.0.checked_add(duration).map(SystemTime)
    }

    /// Returns `Some(t)` where `t` is the time `self - duration` if `t` can be represented as
    /// `SystemTime` (which means it's inside the bounds of the underlying data structure), `None`
    /// otherwise.
    pub fn checked_sub(&self, duration: Duration) -> Option<SystemTime> {
        self.0.checked_sub(duration).map(SystemTime)
    }
}

impl Add<Duration> for SystemTime {
    type Output = SystemTime;

    /// # Panics
    ///
    /// This function may panic if the resulting point in time cannot be represented by the
    /// underlying data structure. See [`SystemTime::checked_add`] for a version without panic.
    fn add(self, dur: Duration) -> SystemTime {
        self.checked_add(dur)
            .expect("overflow when adding duration to instant")
    }
}

impl AddAssign<Duration> for SystemTime {
    fn add_assign(&mut self, other: Duration) {
        *self = *self + other;
    }
}

impl Sub<Duration> for SystemTime {
    type Output = SystemTime;

    fn sub(self, dur: Duration) -> SystemTime {
        self.checked_sub(dur)
            .expect("overflow when subtracting duration from instant")
    }
}

impl SubAssign<Duration> for SystemTime {
    fn sub_assign(&mut self, other: Duration) {
        *self = *self - other;
    }
}

impl fmt::Debug for SystemTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// -----------------------------------------------------------------------------
// SystemTimeError

/// An error returned from the `duration_since` and `elapsed` methods on
/// `SystemTime`, used to learn how far in the opposite direction a system time
/// lies.
#[derive(Clone, Debug)]
pub struct SystemTimeError(Duration);

impl SystemTimeError {
    /// Returns the positive duration which represents how far forward the
    /// second system time was from the first.
    ///
    /// A `SystemTimeError` is returned from the [`SystemTime::duration_since`]
    /// and [`SystemTime::elapsed`] methods whenever the second system time
    /// represents a point later in time than the `self` of the method call.
    #[must_use]
    pub fn duration(&self) -> Duration {
        self.0
    }
}

impl fmt::Display for SystemTimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "second time provided was later than self")
    }
}

impl core::error::Error for SystemTimeError {}
