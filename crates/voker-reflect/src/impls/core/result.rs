crate::derive::impl_reflect! {
    #[reflect(type_path = "core::result::Result")]
    enum Result<T, E> {
        Ok(T),
        Err(E),
    }
}
