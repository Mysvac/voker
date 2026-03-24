use proc_macro::TokenStream;
use quote::quote;
use syn::DeriveInput;

use crate::{ImplSourceKind, derive_data::ReflectDerive};

/// Shared entry point for `#[derive(Reflect)]` and `impl_reflect!`.
pub(crate) fn match_reflect_impls(ast: DeriveInput, source: ImplSourceKind) -> TokenStream {
    // Parse the type kind, attributes, and field metadata.
    let reflect_derive: ReflectDerive = match ReflectDerive::from_input(&ast, source) {
        Ok(val) => val,
        Err(err) => return err.into_compile_error().into(),
    };

    let reflect_impls: proc_macro2::TokenStream = match reflect_derive {
        ReflectDerive::Struct(info) => crate::impls::impl_struct(&info),
        ReflectDerive::TupleStruct(info) => crate::impls::impl_tuple_struct(&info),
        ReflectDerive::Enum(info) => crate::impls::impl_enum(&info),
        ReflectDerive::UnitStruct(meta) => crate::impls::impl_unit(&meta),
        ReflectDerive::Opaque(meta) => crate::impls::impl_opaque(&meta),
    };

    TokenStream::from(quote! {
        const _: () = {
            #reflect_impls
        };
    })
}
