use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{DeriveInput, parse_quote};

pub fn derive_label(ast: DeriveInput, trait_path: TokenStream2) -> TokenStream {
    if matches!(&ast.data, syn::Data::Union(_)) {
        let message = format!("Cannot derive {trait_path} for unions.");
        return syn::Error::new_spanned(ast, message).into_compile_error().into();
    }

    use crate::full_path::{CloneFP, DebugFP, EqFP, HashFP, SendFP, SyncFP};

    let type_ident = ast.ident;

    let mut generics = ast.generics;
    if generics.type_params().next().is_some() {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote! { Self: #SendFP + #SyncFP + #CloneFP + #DebugFP + #HashFP + #EqFP + 'static });
    } else if generics.lifetimes().next().is_some() {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote! { Self: 'static });
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        const _:() = {
            extern crate alloc;

            impl #impl_generics #trait_path for #type_ident #ty_generics #where_clause {
                fn dyn_clone(&self) -> alloc::boxed::Box<dyn #trait_path> {
                    alloc::boxed::Box::new(#CloneFP::clone(self))
                }
            }
        };
    }
    .into()
}
