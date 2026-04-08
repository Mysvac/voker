use alloc::vec::Vec;
use core::any::TypeId;
use core::fmt::Debug;

use voker_utils::extra::TypeIdMap;

use super::{Resource, ResourceDescriptor};
use super::{ResourceId, ResourceInfo};

// -----------------------------------------------------------------------------
// Resources

/// A registry that manages all resource types in the ECS world.
pub struct Resources {
    infos: Vec<ResourceInfo>,
    mapper: TypeIdMap<ResourceId>,
}

impl Debug for Resources {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.infos, f)
    }
}

impl Resources {
    /// Creates a new empty resource registry.
    #[inline]
    pub(crate) const fn new() -> Self {
        Self {
            infos: Vec::new(),
            mapper: TypeIdMap::new(),
        }
    }

    /// Returns the number of registered resources.
    #[inline]
    #[expect(clippy::len_without_is_empty, reason = "useless")]
    pub const fn len(&self) -> usize {
        self.infos.len()
    }

    /// Looks up a resource ID by its [`TypeId`].
    #[inline]
    pub fn get_id(&self, type_id: TypeId) -> Option<ResourceId> {
        self.mapper.get(type_id).copied()
    }

    /// Returns the resource info for the given ID.
    #[inline]
    pub fn get(&self, id: ResourceId) -> Option<&ResourceInfo> {
        self.infos.get(id.index())
    }

    /// Returns the resource info for the given ID without bounds checking.
    ///
    /// # Safety
    /// The caller must ensure `id` is a valid ID (i.e., `id.index() < self.len()`).
    #[inline]
    pub unsafe fn get_unchecked(&self, id: ResourceId) -> &ResourceInfo {
        debug_assert!(id.index() < self.infos.len());
        unsafe { self.infos.get_unchecked(id.index()) }
    }

    /// Extracts a slice containing the entire Components.
    #[inline]
    pub fn as_slice(&self) -> &[ResourceInfo] {
        self.infos.as_slice()
    }

    /// Returns an iterator over the ResourceInfos.
    #[inline]
    pub fn iter(&self) -> core::slice::Iter<'_, ResourceInfo> {
        self.infos.iter()
    }

    /// Registers a resource type `T` and returns its unique ID.
    ///
    /// If the resource type is already registered, this returns the existing ID.
    /// Otherwise, it creates a new descriptor, assigns a new ID, and stores the metadata.
    #[inline]
    pub fn register<T: Resource>(&mut self) -> ResourceId {
        // Separate to reduce the stack frame of external functions,
        // thereby improving the execution speed of the hot path.
        #[cold]
        #[inline(never)]
        fn register_internal(this: &mut Resources, func: fn() -> ResourceDescriptor) -> ResourceId {
            let id = ResourceId::new(this.infos.len() as u32);
            let descriptor = func();
            let type_id = descriptor.type_id;

            this.infos.push(ResourceInfo::new(id, descriptor));
            this.mapper.insert(type_id, id);

            id
        }

        if let Some(id) = self.get_id(TypeId::of::<T>()) {
            id
        } else {
            register_internal(self, ResourceDescriptor::new::<T>)
        }
    }
}
