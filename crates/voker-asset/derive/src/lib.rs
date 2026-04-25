#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(clippy::std_instead_of_core, reason = "proc-macro lib")]
#![allow(clippy::std_instead_of_alloc, reason = "proc-macro lib")]

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{DeriveInput, Ident, Member, Path, parse_macro_input};

// -----------------------------------------------------------------------------
// path

fn voker_asset_path() -> Path {
    voker_macro_utils::crate_path!(bevy_asset)
}

fn asset_(voker_asset_path: &Path) -> TokenStream2 {
    quote! { #voker_asset_path::asset::Asset }
}

fn visit_asset_dependencies_(voker_asset_path: &Path) -> TokenStream2 {
    quote! { #voker_asset_path::asset::VisitAssetDependencies }
}

fn erased_asset_id_(voker_asset_path: &Path) -> TokenStream2 {
    quote! { #voker_asset_path::ident::ErasedAssetId }
}

fn as_member(ident: Option<&Ident>, index: usize) -> Member {
    ident.map_or_else(|| Member::from(index), |ident| Member::Named(ident.clone()))
}

// -----------------------------------------------------------------------------
// derive_asset

const DEPENDENCY_ATTRIBUTE: &str = "dependency";

/// Implement the `Asset` trait.
#[proc_macro_derive(Asset, attributes(dependency))]
pub fn derive_asset(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let voker_asset_path = voker_asset_path();
    let asset_ = asset_(&voker_asset_path);

    let struct_name = &ast.ident;
    let (impl_generics, type_generics, where_clause) = &ast.generics.split_for_impl();

    let dependency_visitor = match derive_dependency_visitor_internal(&ast, &voker_asset_path) {
        Ok(dependency_visitor) => dependency_visitor,
        Err(err) => return err.into_compile_error().into(),
    };

    TokenStream::from(quote! {
        impl #impl_generics #asset_ for #struct_name #type_generics #where_clause { }
        #dependency_visitor
    })
}

// -----------------------------------------------------------------------------
// derive_asset_dependency_visitor

/// Implement the `VisitAssetDependencies` trait.
#[proc_macro_derive(VisitAssetDependencies, attributes(dependency))]
pub fn derive_asset_dependency_visitor(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let voker_asset_path: Path = voker_asset_path();

    match derive_dependency_visitor_internal(&ast, &voker_asset_path) {
        Ok(dependency_visitor) => TokenStream::from(dependency_visitor),
        Err(err) => err.into_compile_error().into(),
    }
}

fn derive_dependency_visitor_internal(
    ast: &DeriveInput,
    voker_asset_path: &Path,
) -> Result<proc_macro2::TokenStream, syn::Error> {
    let struct_name = &ast.ident;
    let (impl_generics, type_generics, where_clause) = &ast.generics.split_for_impl();

    let visit_asset_dependencies_ = visit_asset_dependencies_(voker_asset_path);
    let erased_asset_id_ = erased_asset_id_(voker_asset_path);

    let visit_dep =
        |to_read| quote!(#visit_asset_dependencies_::visit_dependencies(#to_read, visit););
    let is_dep_attribute = |a: &syn::Attribute| a.path().is_ident(DEPENDENCY_ATTRIBUTE);
    let field_has_dep = |f: &syn::Field| f.attrs.iter().any(is_dep_attribute);

    let body = match &ast.data {
        syn::Data::Struct(syn::DataStruct { fields, .. }) => {
            let field_visitors = fields
                .iter()
                .enumerate()
                .filter(|(_, f)| field_has_dep(f))
                .map(|(i, field)| as_member(field.ident.as_ref(), i))
                .map(|member| visit_dep(quote!(&self.#member)));
            Some(quote!( #(#field_visitors)* ))
        }
        syn::Data::Enum(data_enum) => {
            let variant_has_dep = |v: &syn::Variant| v.fields.iter().any(field_has_dep);
            let any_case_required = data_enum.variants.iter().any(variant_has_dep);
            let cases = data_enum.variants.iter().filter(|v| variant_has_dep(v));

            let cases = cases.map(|variant| {
                let ident = &variant.ident;
                let field_members = variant
                    .fields
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| field_has_dep(f))
                    .map(|(i, field)| as_member(field.ident.as_ref(), i));

                let field_locals = field_members.clone().map(|m| format_ident!("__self_{}", m));
                let field_visitors = field_locals.clone().map(|i| visit_dep(quote!(#i)));

                quote!( Self::#ident {#(#field_members: #field_locals,)* ..} => { #(#field_visitors)* } )
            });

            any_case_required.then(|| quote!(match self { #(#cases)*, _ => {} }))
        }
        syn::Data::Union(_) => {
            return Err(syn::Error::new(
                Span::call_site(),
                "Asset derive currently doesn't work on unions",
            ));
        }
    };

    // prevent unused variable warning in case there are no dependencies
    let visit = if body.is_none() {
        quote! { _visit }
    } else {
        quote! { visit }
    };

    Ok(quote! {
        impl #impl_generics #visit_asset_dependencies_ for #struct_name #type_generics #where_clause {
            fn visit_dependencies(&self, #visit: &mut impl FnMut(#erased_asset_id_)) {
                #body
            }
        }
    })
}
