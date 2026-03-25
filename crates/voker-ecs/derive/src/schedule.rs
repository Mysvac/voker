use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_quote};

pub(crate) fn impl_derive_schedule_label(ast: DeriveInput) -> TokenStream {
    use crate::path::fp::{CloneFP, DebugFP, EqFP, HashFP, SendFP, SyncFP};
    let voker_ecs_path = crate::path::voker_ecs();
    let schedule_label_ = crate::path::schedule_label_(&voker_ecs_path);
    let macro_utils_ = crate::path::macro_utils_(&voker_ecs_path);

    let type_ident = ast.ident;

    let mut generics = ast.generics.clone();
    if generics.type_params().next().is_some() {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote! { Self: #SendFP + #SyncFP + #DebugFP + #HashFP + #EqFP + 'static });
    } else if generics.lifetimes().next().is_some() {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote! { Self: 'static });
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        const _:() = {
            impl #impl_generics #schedule_label_ for #type_ident #ty_generics #where_clause {
                fn dyn_clone(&self) -> #macro_utils_::Box<dyn #schedule_label_> {
                    #macro_utils_::Box::new(#CloneFP::clone(self))
                }
            }
        };
    }
    .into()
}
