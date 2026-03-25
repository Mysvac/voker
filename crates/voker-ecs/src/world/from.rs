use crate::world::{World, WorldId};

/// Creates an instance of the type this trait is implemented for
/// using data from the supplied [`World`].
pub trait FromWorld {
    /// Creates `Self` using data from the given [`World`].
    fn from_world(world: &World) -> Self;
}

impl<T: Default> FromWorld for T {
    /// Creates `Self` using [`default()`](`Default::default`).
    fn from_world(_world: &World) -> Self {
        T::default()
    }
}

impl FromWorld for WorldId {
    fn from_world(world: &World) -> Self {
        world.id()
    }
}
