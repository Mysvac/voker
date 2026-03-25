use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_quote};

pub(crate) fn impl_derive_message(ast: DeriveInput) -> TokenStream {
    use crate::path::fp::{SendFP, SyncFP};
    let voker_ecs_path = crate::path::voker_ecs();
    let message_ = crate::path::message_(&voker_ecs_path);

    let type_ident = ast.ident;

    let mut generics = ast.generics.clone();
    if generics.type_params().next().is_some() {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote! { Self: #SendFP + #SyncFP + 'static });
    } else if generics.lifetimes().next().is_some() {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote! { Self: 'static });
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        const _:() = {
            impl #impl_generics #message_ for #type_ident #ty_generics #where_clause {}
        };
    }
    .into()
}
