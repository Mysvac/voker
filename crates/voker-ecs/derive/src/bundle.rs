use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, parse_quote};
use syn::{Fields, Ident, Index, Type};

use crate::utils::{contains_any_idents, field_type_constraint};

pub(crate) fn impl_derive_bundle(ast: DeriveInput) -> proc_macro::TokenStream {
    use crate::path::fp::{SendFP, SyncFP};

    let voker_ecs_path = crate::path::voker_ecs();
    let bundle_ = crate::path::bundle_(&voker_ecs_path);
    let component_collector_ = crate::path::component_collector_(&voker_ecs_path);
    let component_writer_ = crate::path::component_writer_(&voker_ecs_path);

    let type_ident = ast.ident;
    let mut generics = ast.generics;

    let type_param_idents: Vec<Ident> = generics
        .type_params()
        .map(|type_param| type_param.ident.clone())
        .collect();

    if !type_param_idents.is_empty() {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote! { Self: #SendFP + #SyncFP + Sized + 'static });
    } else if generics.lifetimes().next().is_some() {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote! { Self: 'static });
    }

    let field_access: Vec<(TokenStream, &Type)> = match &ast.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => fields
                .named
                .iter()
                .map(|field| {
                    let ident = field.ident.as_ref().unwrap();
                    let ty = &field.ty;
                    if contains_any_idents(ty, &type_param_idents) {
                        field_type_constraint(&mut generics, ty, &bundle_);
                    }
                    (quote! { #ident }, ty)
                })
                .collect(),
            Fields::Unnamed(fields) => fields
                .unnamed
                .iter()
                .enumerate()
                .map(|(i, field)| {
                    let index = Index::from(i);
                    let ty = &field.ty;
                    if contains_any_idents(ty, &type_param_idents) {
                        field_type_constraint(&mut generics, ty, &bundle_);
                    }
                    (quote! { #index }, ty)
                })
                .collect(),
            Fields::Unit => {
                let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
                return quote! {
                    const _: () = {
                        #[expect(unsafe_code, reason = "bundle implementation is unsafe.")]
                        unsafe impl #impl_generics #bundle_ for #type_ident #ty_generics #where_clause {
                            fn collect_components(_collector: &mut #component_collector_) {}
                            unsafe fn write_explicit(_writer: &mut #component_writer_, _base: usize) {}
                            unsafe fn write_required(_writer: &mut #component_writer_) {}
                        }
                    };
                }
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(&type_ident, "Bundle can only be derived for structs")
                .into_compile_error()
                .into();
        }
    };

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let collect_calls = field_access.iter().map(|(_, ty)| {
        quote! {
            <#ty as #bundle_>::collect_components(__collector__);
        }
    });

    let write_explicit_calls = field_access.iter().map(|(ident, ty)| {
        quote! {
            unsafe {
                let __offset__ = ::core::mem::offset_of!(Self, #ident) + __base__;
                <#ty as #bundle_>::write_explicit(__writer__, __offset__);
            }
        }
    });

    let write_required_calls = field_access.iter().map(|(_, ty)| {
        quote! {
            unsafe {
                <#ty as #bundle_>::write_required(__writer__);
            }
        }
    });

    quote! {
        const _: () = {
            #[expect(unsafe_code, reason = "bundle implementation is unsafe.")]
            unsafe impl #impl_generics #bundle_ for #type_ident #ty_generics #where_clause {
                fn collect_components(__collector__: &mut #component_collector_) {
                    #(#collect_calls)*
                }

                unsafe fn write_explicit(__writer__: &mut #component_writer_, __base__: usize) {
                    #(#write_explicit_calls)*
                }

                unsafe fn write_required(__writer__: &mut #component_writer_) {
                    #(#write_required_calls)*
                }
            }
        };
    }
    .into()
}
