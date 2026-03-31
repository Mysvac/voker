//! This independent module is used to provide the required path.
//! So as to minimize changes when the `voker_reflect` structure is modified.
//!
//! The only special feature is the path of voker_reflect itself,
//! See [`voker_reflect`] function doc.

use proc_macro2::TokenStream;
use quote::quote;

// -----------------------------------------------------------------------------
// Crate Path

/// Get the correct access path to the `voker_reflect` crate.
///
/// Not all modules can access the reflection crate itself through `voker_reflect`,
/// we have to scan the builder's `cargo.toml`.
///
/// 1. For crates that depend on `voker_reflect`, `::voker_reflect` is returned here`.
/// 2. For crates that depend on `voker`, `::voker::reflect` is returned here`.
/// 3. For crates that depend on `void_craft`, `::void_craft::reflect` is returned here`.
/// 4. For crates that depend on `vc`, `::vc::reflect` is returned here`.
/// 5. For other situations, `::voker_reflect` is returned here, but this may be incorrect.
///
/// The cost of this function is relatively high (accessing files, obtaining read-write lock permissions, querying content...),
/// so the crate path is mainly obtained through parameter passing rather than reacquiring.
pub(crate) fn voker_reflect() -> syn::Path {
    voker_macro_utils::crate_path!(voker_reflect)
}

pub(crate) use voker_macro_utils::full_path as fp;

// -----------------------------------------------------------------------------
// Modules

mod cell;
mod info;
mod ops;
mod registry;

// -----------------------------------------------------------------------------
// Internal API

pub(crate) use cell::*;
pub(crate) use info::*;
pub(crate) use ops::*;
pub(crate) use registry::*;

#[inline(always)]
pub(crate) fn auto_register_(
    voker_reflect_path: &syn::Path,
    span: ::proc_macro2::Span,
) -> TokenStream {
    let auto_register = ::syn::Ident::new("auto_register", span);
    quote! {
        #voker_reflect_path::__macro_exports::#auto_register
    }
}

// mod access;
// `voker_reflect::access` does not require additional content.

#[inline(always)]
pub(crate) fn macro_utils_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::__macro_exports::macro_utils
    }
}

#[inline(always)]
pub(crate) fn reflect_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::Reflect
    }
}

#[inline(always)]
pub(crate) fn from_reflect_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::FromReflect
    }
}

#[inline(always)]
pub(crate) fn reflect_hasher_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::reflect_hasher
    }
}
