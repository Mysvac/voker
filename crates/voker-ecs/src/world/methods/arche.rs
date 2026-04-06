use core::ptr::NonNull;

use alloc::vec::Vec;

use voker_utils::hash::SparseHashSet;

use crate::archetype::ArcheId;
use crate::bundle::BundleId;
use crate::component::{CollectResult, ComponentCollector, ComponentId};
use crate::world::World;

impl World {
    /// Returns the archetype ID after inserting a bundle into the given archetype.
    ///
    /// This is primarily used when inserting components into an entity to determine
    /// whether its archetype changes. If the target archetype does not exist yet,
    /// it will be created, so this function requires mutable access to [`World`].
    pub fn arche_after_insert(&mut self, arche_id: ArcheId, bundle_id: BundleId) -> ArcheId {
        let arche = unsafe { self.archetypes.get_unchecked(arche_id) };
        if let Some(cached) = arche.after_insert(bundle_id) {
            return cached;
        }
        arche_after_insert_slow(self, arche_id, bundle_id)
    }

    /// Returns the archetype ID after removing a bundle from the given archetype.
    ///
    /// This function remains valid even if some components in the bundle are not
    /// present in the archetype; missing components are ignored.
    ///
    /// This is primarily used when removing components from an entity to determine
    /// whether its archetype changes. If the target archetype does not exist yet,
    /// it will be created, so this function requires mutable access to [`World`].
    pub fn arche_after_remove(&mut self, arche_id: ArcheId, bundle_id: BundleId) -> ArcheId {
        let arche = unsafe { self.archetypes.get_unchecked(arche_id) };
        if let Some(cached) = arche.after_remove(bundle_id) {
            return cached;
        }
        arche_after_remove_slow(self, arche_id, bundle_id)
    }
}

#[cold]
#[inline(never)]
fn arche_after_insert_slow(this: &mut World, arche_id: ArcheId, bundle_id: BundleId) -> ArcheId {
    let arche = unsafe { this.archetypes.get_unchecked(arche_id) };
    let bundle = unsafe { this.bundles.get_unchecked(bundle_id) };

    let dense_upper = arche.dense_components().len() + bundle.dense_components().len();
    let sparse_upper = arche.sparse_components().len() + bundle.sparse_components().len();

    // `dense` will later append `sparse`, so pre-allocate full capacity.
    let mut dense: Vec<ComponentId> = Vec::with_capacity(dense_upper + sparse_upper);
    dense.extend_from_slice(arche.dense_components());
    dense.extend_from_slice(bundle.dense_components());
    dense.sort();
    dense.dedup();

    let mut sparse: Vec<ComponentId> = Vec::with_capacity(sparse_upper);
    sparse.extend_from_slice(arche.sparse_components());
    sparse.extend_from_slice(bundle.sparse_components());
    sparse.sort();
    sparse.dedup();

    let dense_len = dense.len();
    dense.append(&mut sparse);

    if let Some(result) = this.archetypes.get_id(&dense) {
        let arche = unsafe { this.archetypes.get_unchecked_mut(arche_id) };
        arche.set_after_insert(bundle_id, result);
        return result;
    }

    let idents = crate::utils::SlicePool::component(&dense);

    let table_id = unsafe {
        let sparse: &[ComponentId] = &idents[dense_len..];
        this.storages.maps.register(&this.components, sparse);
        let denses: &[ComponentId] = &idents[..dense_len];
        this.storages.tables.register(&this.components, denses)
    };

    unsafe {
        let result = this
            .archetypes
            .register_unique(table_id, dense_len, idents, &this.components);
        let arche = this.archetypes.get_unchecked_mut(arche_id);
        arche.set_after_insert(bundle_id, result);
        result
    }
}

#[cold]
#[inline(never)]
fn arche_after_remove_slow(this: &mut World, arche_id: ArcheId, bundle_id: BundleId) -> ArcheId {
    let arche = unsafe { this.archetypes.get_unchecked(arche_id) };
    let bundle = unsafe { this.bundles.get_unchecked(bundle_id) };

    let dense_upper = arche.dense_components().len();
    let mut dense: SparseHashSet<ComponentId> = SparseHashSet::with_capacity(dense_upper);
    dense.extend(arche.dense_components());
    bundle.dense_components().iter().for_each(|&id| {
        dense.remove(&id);
    });

    let sparse_upper = arche.sparse_components().len();
    let mut sparse: SparseHashSet<ComponentId> = SparseHashSet::with_capacity(sparse_upper);
    sparse.extend(arche.sparse_components());
    bundle.sparse_components().iter().for_each(|&id| {
        sparse.remove(&id);
    });

    // HACK: `Collector` requires a mutable reference to `Components`, but accessing
    // a component's `Required` needs a shared reference, violating aliasing rules.
    // Using raw pointers to bypass this and avoid complexity. The caller ensures
    // safety as the internal implementation is deterministic.
    let mut ptr = NonNull::from_mut(&mut this.components);
    let mut collector = ComponentCollector::new(unsafe { ptr.as_mut() });
    dense.iter().chain(sparse.iter()).for_each(|&id| {
        let info = unsafe { ptr.as_ref().get_unchecked(id) };
        if let Some(required) = info.required() {
            required.collect(&mut collector);
        }
    });

    let CollectResult {
        dense: mut dense_vec,
        sparse: mut sparse_vec,
    } = collector.unsorted();

    dense_vec.extend(dense);
    dense_vec.sort();
    dense_vec.dedup();

    sparse_vec.extend(sparse);
    sparse_vec.sort();
    sparse_vec.dedup();

    let dense_len = dense_vec.len();
    dense_vec.append(&mut sparse_vec);

    if let Some(result) = this.archetypes.get_id(&dense_vec) {
        let arche = unsafe { this.archetypes.get_unchecked_mut(arche_id) };
        arche.set_after_remove(bundle_id, result);
        return result;
    }

    let idents = crate::utils::SlicePool::component(&dense_vec);

    let table_id = unsafe {
        let sparse: &[ComponentId] = &idents[dense_len..];
        this.storages.maps.register(&this.components, sparse);
        let denses: &[ComponentId] = &idents[..dense_len];
        this.storages.tables.register(&this.components, denses)
    };

    unsafe {
        let result = this
            .archetypes
            .register_unique(table_id, dense_len, idents, &this.components);
        let arche = this.archetypes.get_unchecked_mut(arche_id);
        arche.set_after_remove(bundle_id, result);
        result
    }
}
