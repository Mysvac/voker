//! This independent module is used to provide the required path.
//! So as to minimize changes when the `voker_ecs` structure is modified.

use proc_macro2::TokenStream;
use quote::quote;

// -----------------------------------------------------------------------------
// Crate Path

/// Get the correct access path to the `voker_ecs` crate.
pub(crate) fn voker_ecs() -> syn::Path {
    voker_macro_utils::crate_path!(voker_ecs)
}

pub(crate) use voker_macro_utils::full_path as fp;

#[inline(always)]
pub(crate) fn macro_utils_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::__macro_exports::macro_utils
    }
}

// -----------------------------------------------------------------------------
// Resource

#[inline(always)]
pub(crate) fn cloner_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::utils::Cloner
    }
}

#[inline(always)]
pub(crate) fn resource_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::resource::Resource
    }
}

#[inline(always)]
pub(crate) fn component_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::component::Component
    }
}

#[inline(always)]
pub(crate) fn required_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::component::Required
    }
}

#[inline(always)]
pub(crate) fn storage_mode_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::component::StorageMode
    }
}

#[inline(always)]
pub(crate) fn component_collector_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::component::ComponentCollector
    }
}

#[inline(always)]
pub(crate) fn component_writer_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::component::ComponentWriter
    }
}

#[inline(always)]
pub(crate) fn component_hook_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::component::ComponentHook
    }
}

#[inline(always)]
pub(crate) fn bundle_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::bundle::Bundle
    }
}

#[inline(always)]
pub(crate) fn schedule_label_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::schedule::ScheduleLabel
    }
}

#[inline(always)]
pub(crate) fn message_(voker_ecs_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_ecs_path::message::Message
    }
}
