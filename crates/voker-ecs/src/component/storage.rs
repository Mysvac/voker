/// Represents the storage mode for component data.
///
/// Two storage modes are provided:
///
/// - `Dense`: Dense storage (default), corresponding to `Table` in Storages.
/// - `Sparse`: Sparse storage, corresponding to `Map` in Storages.
///
/// Both modes provide O(1) access to component data when holding an `Entity` handle,
/// with the main difference being in query performance.
///
/// # Dense Storage
///
/// Dense storage uses a table structure:
///
/// |  TableId  | Component A | Component B | Component C | .. |
/// |-----------|-------------|-------------|-------------|----|
/// | Entity A  | /* data */  | /* data */  | /* data */  | .. |
/// | Entity B  | /* data */  | /* data */  | /* data */  | .. |
/// | Entity C  | /* data */  | /* data */  | /* data */  | .. |
/// | ........  | ..........  | ..........  | ..........  | .. |
///
/// This structure provides optimal cache locality during iteration. However, it has a
/// fundamental limitation: tables have a fixed schema. When entities need to add or remove
/// components, the straightforward solution would require moving entire rows (entities)
/// between tables, which is inefficient.
///
/// A common mitigation is to pre-design tables with `Option<Component<T>>` columns:
/// - Insertion: set the value to `Some(data)`
/// - Removal: set the value to `None`
///
/// This avoids moving entities between tables but introduces three problems:
///
/// 1. All possible components must be known at table creation time
/// 2. Numerous `None` values lead to significant memory waste
/// 3. Cannot query for component presence in O(1); requires full table traversal
///
/// # Sparse Storage
///
/// To address these limitations, sparse storage is provided:
///
/// |    Component A     |    Component B     |    Component C    | .. |
/// |--------------------|--------------------|-------------------|----|
/// | Map<Entity, Data>  | Map<Entity, Data>  | Map<Entity, Data> | .. |
///
/// Key characteristics:
/// 1. Each sparse component exists in its own independent `Map`, allowing component
///    addition/removal without affecting other data.
/// 2. Maps maintain entity sets, enabling efficient filtering of entities possessing
///    specific components.
///
/// # Query Performance
///
/// While both modes offer O(1) access when holding an `Entity` handle (entities store
/// their table row index), query performance differs significantly:
///
/// - **Queries without sparse components**: Iterators traverse tables row by row,
///   achieving high cache hit rates.
/// - **Queries with sparse components**: Iterators must first filter entity sets,
///   then iterate over entities. Each iteration results in random access, significantly
///   reducing cache efficiency.
///
/// # Recommendation
///
/// **Prefer `Dense` storage over `Sparse` whenever possible.**
///
/// Sparse storage should be reserved for special flag components or rarely-present data.
/// Additionally, sparse components should not be held by large numbers of entities to
/// minimize random access during queries.
#[derive(Default, Debug, Clone, Copy)]
pub enum ComponentStorage {
    #[default]
    Dense = 0,
    Sparse = 1,
}

impl ComponentStorage {
    #[inline]
    pub const fn is_dense(self) -> bool {
        self as u8 == ComponentStorage::Dense as u8
    }

    #[inline]
    pub const fn is_sparse(self) -> bool {
        self as u8 == ComponentStorage::Sparse as u8
    }
}
