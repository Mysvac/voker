//! Path helpers for generated code.
//! Keeping all crate paths here minimizes macro changes when crate layout changes.

use proc_macro2::TokenStream;
use quote::quote;

/// Resolve the effective path to `voker_state` for macro expansion.
pub(crate) fn voker_state_path() -> syn::Path {
    voker_macro_utils::crate_path!(voker_state)
}

pub(crate) use voker_macro_utils::full_path as fp;

#[inline(always)]
pub(crate) fn states_(voker_state_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_state_path::state::States
    }
}

#[inline(always)]
pub(crate) fn manual_state_(voker_state_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_state_path::state::ManualState
    }
}

#[inline(always)]
pub(crate) fn sub_states_(voker_state_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_state_path::state::SubStates
    }
}
