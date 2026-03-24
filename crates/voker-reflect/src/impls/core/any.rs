crate::derive::impl_reflect_opaque!(::core::any::TypeId(
    clone,
    debug,
    hash,
    eq,
    cmp,
    auto_register,
));

#[cfg(test)]
mod tests {
    use crate::FromReflect;

    #[test]
    fn type_id_should_from_reflect() {
        let type_id = core::any::TypeId::of::<usize>();
        let output = <core::any::TypeId as FromReflect>::from_reflect(&type_id).unwrap();
        assert_eq!(type_id, output);
    }
}
