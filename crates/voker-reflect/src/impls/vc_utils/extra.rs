use crate::{derive::impl_type_path, info::TypePath};

impl_type_path!(::voker_utils::extra::TypeIdMap<T> where T: TypePath);
impl_type_path!(::voker_utils::extra::BlockList<T> where T: TypePath);
impl_type_path!(::voker_utils::extra::ArrayDeque<T, const N: usize> where T: TypePath);
