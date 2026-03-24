//! Optional send for wasm support

use alloc::boxed::Box;
use core::pin::Pin;

// -----------------------------------------------------------------------------
// Internal

crate::cfg::web! {
    if {
        /// Use [`CondSend`] to mark an optional Send trait bound.
        /// Useful as on certain platforms (eg. Wasm), futures aren't Send.
        pub trait CondSend {}
        impl<T> CondSend for T {}
    } else {
        /// Use [`CondSend`] to mark an optional Send trait bound.
        /// Useful as on certain platforms (eg. Wasm), futures aren't Send.
        pub trait CondSend: Send {}
        impl<T: Send> CondSend for T {}
    }
}

impl<T: Future + CondSend> CondSendFuture for T {}

// -----------------------------------------------------------------------------
// Exports

/// Use [`CondSendFuture`] for a future with an optional Send trait bound,
/// as on certain platforms (eg. Wasm), futures aren't Send.
pub trait CondSendFuture: Future + CondSend {}

/// An owned and dynamically typed Future used when you can't
/// statically type your result or need to add some indirection.
pub type BoxedFuture<'a, T> = Pin<Box<dyn CondSendFuture<Output = T> + 'a>>;
