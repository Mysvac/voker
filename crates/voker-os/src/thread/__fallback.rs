use crate::time::{Instant, Duration};

/// Puts the current thread to sleep for at least the specified amount of time.
///
/// As this is a `no_std` fallback implementation, this will spin the current thread.
/// 
/// This API depends on [`time::Instant`](crate::time::Instant), 
/// If used in non no_std environments, remember to set elapsed getter.
pub fn sleep(dur: Duration) {
    let start = Instant::now();

    while start.elapsed() < dur {
        core::hint::spin_loop();
    }
}
