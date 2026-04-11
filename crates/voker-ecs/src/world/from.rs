use super::{World, WorldId};

pub trait FromWorld {
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
