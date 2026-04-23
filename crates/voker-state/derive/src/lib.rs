#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(clippy::std_instead_of_core, reason = "proc-macro lib")]
#![allow(clippy::std_instead_of_alloc, reason = "proc-macro lib")]

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{DeriveInput, Expr, Ident, Token, Type, parse_macro_input};
use syn::{parse::Parse, parse::ParseStream};

mod path;

// -----------------------------------------------------------------------------
// derive_states

#[proc_macro_derive(States)]
pub fn derive_states(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let voker_state_path = path::voker_state_path();
    let states_ = path::states_(&voker_state_path);
    let manual_states_ = path::manual_states_(&voker_state_path);

    let ident = ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    quote! {
        impl #impl_generics #states_ for #ident #ty_generics #where_clause {}

        impl #impl_generics #manual_states_ for #ident #ty_generics #where_clause {}
    }
    .into()
}

// -----------------------------------------------------------------------------
// derive_sub_states

struct SourceAttr {
    source: Type,
    _eq: Token![=],
    value: Expr,
}

impl Parse for SourceAttr {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        Ok(Self {
            source: input.parse()?,
            _eq: input.parse()?,
            value: input.parse()?,
        })
    }
}

#[proc_macro_derive(SubStates, attributes(source))]
pub fn derive_sub_states(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let voker_state_path = path::voker_state_path();
    let states_ = path::states_(&voker_state_path);
    let manual_states_ = path::manual_states_(&voker_state_path);
    let sub_states_ = path::sub_states_(&voker_state_path);
    let state_set_ = path::state_set_(&voker_state_path);

    let mut source_attr: Option<(Type, Expr)> = None;
    for attr in &ast.attrs {
        if attr.path().is_ident("source") {
            let parsed: SourceAttr = match attr.parse_args::<SourceAttr>() {
                Ok(v) => v,
                Err(e) => {
                    return e.into_compile_error().into();
                }
            };

            source_attr = Some((parsed.source, parsed.value));
            break;
        }
    }

    let ident: Ident = ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    let Some((source_type, source_value)) = source_attr else {
        return syn::Error::new(
            Span::call_site(),
            "missing #[source(SourceType = value)] on SubStates derive",
        )
        .into_compile_error()
        .into();
    };

    quote! {
        impl #impl_generics #states_ for #ident #ty_generics #where_clause {
            const DEPENDENCY_DEPTH : usize = <<Self as #sub_states_>::SourceStates as #state_set_>::STATE_SET_DEPENDENCY_DEPTH + 1;
        }

        impl #impl_generics #manual_states_ for #ident #ty_generics #where_clause {}

        impl #impl_generics #sub_states_ for #ident #ty_generics #where_clause {
            type SourceStates = #source_type;

            fn should_exist(sources: <Self as #sub_states_>::SourceStates) -> Option<Self> {
                matches!(sources, #source_value).then_some(Self::default())
            }
        }
    }
    .into()
}
