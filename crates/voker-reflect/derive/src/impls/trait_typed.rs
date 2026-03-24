use proc_macro2::TokenStream;
use quote::quote;

use crate::derive_data::ReflectMeta;

/// Generate implementation code for `Typed`
///
/// For param `type_info_tokens`, See the `to_info_tokens` of [`ReflectMeta`], [`ReflectStruct`] and [`ReflectEnum`].
///
/// For param `add_from_reflect`, See [`ReflectMeta::split_generics`]
///
/// [`ReflectStruct`]: crate::derive_data::ReflectStruct
/// [`ReflectEnum`]: crate::derive_data::ReflectEnum
pub(crate) fn impl_trait_typed(
    meta: &ReflectMeta,
    type_info_tokens: TokenStream,
    is_const_expr: bool,
    add_from_reflect: bool,
) -> TokenStream {
    let voker_reflect_path = meta.voker_reflect_path();
    let trait_typed_ = crate::path::typed_(voker_reflect_path);
    let type_info_ = crate::path::type_info_(voker_reflect_path);

    let inner_cell_tokens = if meta.contains_generics() {
        let info_cell = crate::path::generic_type_info_cell_(voker_reflect_path);
        quote! {
            static CELL: #info_cell = #info_cell::new();
            CELL.get_or_insert::<Self>(|| { #type_info_tokens })
        }
    } else if is_const_expr {
        quote! {
            static INFO: #type_info_ = #type_info_tokens;
            &INFO
        }
    } else {
        let info_cell = crate::path::non_generic_type_info_cell_(voker_reflect_path);
        quote! {
            static CELL: #info_cell = #info_cell::new();
            CELL.get_or_init(|| { #type_info_tokens })
        }
    };

    let real_ident = meta.real_ident();
    let (impl_generics, ty_generics, where_clause) =
        meta.split_generics(true, false, add_from_reflect);

    quote! {
        impl #impl_generics #trait_typed_ for #real_ident #ty_generics #where_clause {
            fn type_info() -> &'static #type_info_ {
                #inner_cell_tokens
            }
        }
    }
}
