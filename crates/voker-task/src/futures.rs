//! Utilities for working with [`Future`]s.

use core::future::Future;
use core::pin::pin;
use core::task::{Context, Poll, Waker};

/// Consumes a future, polls it once, and immediately returns the output
/// or returns `None` if it wasn't ready yet.
///
/// This will cancel the future if it's not ready.
///
/// # Examples
///
/// ```
/// use core::future::ready;
/// use voker_task::futures::now_or_never;
///
/// assert_eq!(now_or_never(ready(42)), Some(42));
/// ```
pub fn now_or_never<F: Future>(future: F) -> Option<F::Output> {
    let mut cx = Context::from_waker(Waker::noop());
    match pin!(future).poll(&mut cx) {
        Poll::Ready(x) => Some(x),
        _ => None,
    }
}

/// Polls a future once, and returns the output if ready
/// or returns `None` if it wasn't ready yet.
///
/// # Examples
///
/// ```
/// use core::future::ready;
/// use voker_task::futures::check_ready;
///
/// let mut future = ready("done");
/// assert_eq!(check_ready(&mut future), Some("done"));
/// ```
pub fn check_ready<F: Future + Unpin>(future: &mut F) -> Option<F::Output> {
    now_or_never(future)
}
