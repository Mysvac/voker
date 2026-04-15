mod ident;
mod system;

mod builder;

pub use ident::{ObservedBy, ObserverId};
pub use system::{IntoObserverSystem, ObserverSystem, On};

mod observer;
mod observers;

pub use builder::*;
pub use observer::*;
pub use observers::*;
