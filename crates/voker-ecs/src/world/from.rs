use super::{World, WorldId};

/// Constructs a value from immutable world context.
///
/// This is commonly used for resource-style initialization paths that need to
/// derive defaults from world state.
pub trait FromWorld {
    /// Creates `Self` using data available from [`World`].
    fn from_world(world: &World) -> Self;
}

impl<T: Default> FromWorld for T {
    fn from_world(_world: &World) -> Self {
        T::default()
    }
}

impl FromWorld for WorldId {
    fn from_world(world: &World) -> Self {
        world.id()
    }
}
