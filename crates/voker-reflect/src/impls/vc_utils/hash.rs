use crate::derive::impl_type_path;
use crate::impls::{impl_reflect_for_fixedhashmap, impl_reflect_for_fixedhashset};
use crate::impls::{impl_reflect_for_hashmap, impl_reflect_for_hashset};

// -----------------------------------------------------------------------------
// HashState Hasher

impl_type_path!(::voker_utils::hash::FixedHashState);
impl_type_path!(::voker_utils::hash::NoOpHashState);
impl_type_path!(::voker_utils::hash::SparseHashState);

impl_type_path!(
    (in foldhash::fast as RandomState)
    ::voker_utils::hash::foldhash::fast::RandomState
);

impl_type_path!(
    (in foldhash::fast as FixedState)
    ::voker_utils::hash::foldhash::fast::FixedState
);

impl_type_path!(
    (in foldhash::quality as RandomState)
    ::voker_utils::hash::foldhash::quality::RandomState
);

impl_type_path!(
    (in foldhash::quality as FixedState)
    ::voker_utils::hash::foldhash::quality::FixedState
);

impl_type_path!(
    (in hashbrown as DefaultHashBuilder)
    ::voker_utils::hash::hashbrown::DefaultHashBuilder
);

// -----------------------------------------------------------------------------
// Fixed HashSet and HashMap

impl_type_path!(::voker_utils::hash::HashSet<T, S>);
impl_type_path!(::voker_utils::hash::HashMap<K, V, S>);

impl_reflect_for_hashset!(
    ::voker_utils::hash::HashSet<T, S>,
    ::voker_utils::hash::FixedHashState,
);

impl_reflect_for_hashmap!(
    ::voker_utils::hash::HashMap<K, V, S>,
    ::voker_utils::hash::FixedHashState,
);

// -----------------------------------------------------------------------------
// NoOp HashSet and HashMap

impl_type_path!(::voker_utils::hash::NoOpHashSet<T>);
impl_type_path!(::voker_utils::hash::NoOpHashMap<K, V>);

impl_reflect_for_fixedhashset!(::voker_utils::hash::NoOpHashSet<T>);

impl_reflect_for_fixedhashmap!(
    ::voker_utils::hash::NoOpHashMap<K, V>
);

// -----------------------------------------------------------------------------
// Sparse HashSet and HashMap

impl_type_path!(::voker_utils::hash::SparseHashSet<T>);
impl_type_path!(::voker_utils::hash::SparseHashMap<K, V>);

impl_reflect_for_fixedhashset!(::voker_utils::hash::SparseHashSet<T>);

impl_reflect_for_fixedhashmap!(
    ::voker_utils::hash::SparseHashMap<K, V>
);

// // -----------------------------------------------------------------------------
// // hashbrown HashSet and HashMap
//
// impl_type_path!(
//     (in hashbrown as HashSet)
//     ::voker_utils::hash::hashbrown::HashSet<T, S>
// );
//
// impl_type_path!(
//     (in hashbrown as HashMap)
//     ::voker_utils::hash::hashbrown::HashMap<K, V, S>
// );
//
// impl_reflect_for_hashset!(
//     ::voker_utils::hash::hashbrown::HashSet<T, S>,
//     ::voker_utils::hash::hashbrown::DefaultHashBuilder,
// );
//
// impl_reflect_for_hashmap!(
//     ::voker_utils::hash::hashbrown::HashMap<K, V, S>,
//     ::voker_utils::hash::hashbrown::DefaultHashBuilder,
// );
