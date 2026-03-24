use crate::{derive::impl_type_path, impls::{impl_reflect_for_hashmap, impl_reflect_for_hashset}};

impl_type_path!(::std::hash::RandomState);
impl_type_path!(::std::collections::HashSet<T, S>);
impl_type_path!(::std::collections::HashMap<K, V, S>);

impl_reflect_for_hashset!(::std::collections::HashSet<T, S>, ::std::hash::RandomState);
impl_reflect_for_hashmap!(::std::collections::HashMap<K, V, S>, ::std::hash::RandomState);
