#![cfg_attr(docsrs, feature(doc_cfg))]
#![no_std]

extern crate alloc;
extern crate std;

pub mod asset;
pub mod changes;
pub mod handle;
pub mod ident;
pub mod io;
pub mod loader;
pub mod meta;
pub mod path;
pub mod render_asset;
pub mod server;

mod utils;

// -----------------------------------------------------------------------------
// Inline

use alloc::boxed::Box;
use core::pin::Pin;
use core::task::Poll;
use futures_lite::Stream;
use std::path::PathBuf;

pub type BoxedFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type PathStream = dyn Stream<Item = PathBuf> + Unpin + Send;

/// A [`PathBuf`] [`Stream`] implementation that immediately returns nothing.
pub struct EmptyPathStream;

impl Stream for EmptyPathStream {
    type Item = PathBuf;
    #[inline(always)]
    fn poll_next(
        self: Pin<&mut Self>,
        _cx: &mut core::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        Poll::Ready(None)
    }
}
