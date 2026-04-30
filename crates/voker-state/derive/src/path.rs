//! Path helpers for generated code.
//! Keeping all crate paths here minimizes macro changes when crate layout changes.

use proc_macro2::TokenStream;
use quote::quote;

/// Resolve the effective path to `voker_state` for macro expansion.
pub(crate) fn voker_state_path() -> syn::Path {
    voker_macro_utils::crate_path!(voker_state)
}

#[inline(always)]
pub(crate) fn states_(voker_state_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_state_path::state::States
    }
}

#[inline(always)]
pub(crate) fn manual_states_(voker_state_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_state_path::state::ManualStates
    }
}

#[inline(always)]
pub(crate) fn sub_states_(voker_state_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_state_path::state::SubStates
    }
}

#[inline(always)]
pub(crate) fn state_set_(voker_state_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_state_path::state::StateSet
    }
}
