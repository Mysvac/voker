use proc_macro2::TokenStream;
use quote::quote;

use crate::derive_data::ReflectMeta;
use crate::path::fp::OptionFP;
use crate::string_expr::StringExpr;

fn static_path_cell(voker_reflect_path: &syn::Path, generator: TokenStream) -> TokenStream {
    let path_cell_ = crate::path::generic_type_path_cell_(voker_reflect_path);

    quote! {
        static CELL: #path_cell_ = #path_cell_::new();
        CELL.get_or_insert::<Self>(|| { #generator })
    }
}

fn wrap_in_option(tokens: Option<TokenStream>) -> TokenStream {
    use crate::path::fp::OptionFP;
    match tokens {
        Some(tokens) => quote! {
            #OptionFP::Some(#tokens)
        },
        None => quote! {
            #OptionFP::None
        },
    }
}

/// Generate implementation codes for `TypePath`
pub(crate) fn impl_trait_type_path(meta: &ReflectMeta) -> TokenStream {
    let voker_reflect_path = meta.voker_reflect_path();
    let trait_type_path_ = crate::path::type_path_(voker_reflect_path);

    let real_ident = meta.real_ident();

    let (type_path, type_name, inline_flag) = if meta.has_type_const_generics() {
        (
            static_path_cell(voker_reflect_path, meta.type_path_into_owned()),
            static_path_cell(voker_reflect_path, meta.type_name_into_owned()),
            TokenStream::new(),
        )
    } else {
        (
            meta.type_path().into_borrowed(),
            meta.type_name().into_borrowed(),
            quote! { #[inline] },
        )
    };

    let type_ident = meta.type_ident().into_borrowed();
    let module_path = wrap_in_option(meta.module_path().map(StringExpr::into_borrowed));
    let (impl_generics, ty_generics, where_clause) = meta.split_generics(false, false, false);

    quote! {
        impl #impl_generics #trait_type_path_ for #real_ident #ty_generics #where_clause {
            #inline_flag
            fn type_path() -> &'static str {
                #type_path
            }

            #inline_flag
            fn type_name() -> &'static str {
                #type_name
            }

            #[inline]
            fn type_ident() -> &'static str {
                #type_ident
            }

            #[inline]
            fn module_path() -> #OptionFP<&'static str> {
                #module_path
            }
        }
    }
}
