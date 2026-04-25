use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, parse_quote};
use syn::{Fields, Index, Type};

use crate::utils::field_type_constraint;

pub(crate) fn impl_derive_bundle(ast: DeriveInput) -> proc_macro::TokenStream {
    use crate::path::fp::{SendFP, SyncFP};

    let voker_ecs_path = crate::path::voker_ecs();
    let bundle_ = crate::path::bundle_(&voker_ecs_path);
    let data_bundle_ = crate::path::data_bundle_(&voker_ecs_path);
    let component_collector_ = crate::path::component_collector_(&voker_ecs_path);
    let component_writer_ = crate::path::component_writer_(&voker_ecs_path);
    let entity_owned_ = crate::path::entity_owned_(&voker_ecs_path);
    let owning_ptr_ = crate::path::owning_ptr_(&voker_ecs_path);

    // Detect #[bundle(effect)] — opts into Bundle (instead of DataBundle) field
    // constraints and sets NEED_APPLY_EFFECT = true.
    let mut has_effect = false;
    for attr in ast.attrs.iter() {
        if attr.path().is_ident("bundle") {
            let Ok(param) = attr.parse_args::<syn::Ident>() else {
                return syn::Error::new_spanned(attr, "expected `bundle(effect)`")
                    .into_compile_error()
                    .into();
            };
            if param == "effect" {
                has_effect = true;
            } else {
                return syn::Error::new_spanned(attr, "expected `bundle(effect)`")
                    .into_compile_error()
                    .into();
            }
        }
    }

    let type_ident = ast.ident;
    let mut generics = ast.generics;

    if generics.type_params().next().is_some() {
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

    // Default: fields must be DataBundle (no side-effects).
    // With #[bundle(effect)]: fields only need Bundle.
    let field_constraint = if has_effect { &bundle_ } else { &data_bundle_ };

    let field_access: Vec<(TokenStream, &Type)> = match &ast.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => fields
                .named
                .iter()
                .map(|field| {
                    let ident = field.ident.as_ref().unwrap();
                    let ty = &field.ty;
                    field_type_constraint(&mut generics, ty, field_constraint);
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
                    field_type_constraint(&mut generics, ty, field_constraint);
                    (quote! { #index }, ty)
                })
                .collect(),
            Fields::Unit => {
                let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
                return quote! {
                    const _: () = {
                        #[expect(unsafe_code, reason = "bundle implementation is unsafe.")]
                        unsafe impl #impl_generics #bundle_ for #type_ident #ty_generics #where_clause {
                            const NEED_APPLY_EFFECT: bool = false;
                            fn collect_explicit(_collector: &mut #component_collector_) {}
                            fn collect_required(_collector: &mut #component_collector_) {}
                            unsafe fn write_explicit(_data: #owning_ptr_<'_>, _writer: &mut #component_writer_) {}
                            unsafe fn write_required(_writer: &mut #component_writer_) {}
                            unsafe fn apply_effect(_ptr: #owning_ptr_<'_>, _entity: &mut #entity_owned_) {}
                        }
                        #[expect(unsafe_code, reason = "bundle implementation is unsafe.")]
                        unsafe impl #impl_generics #data_bundle_ for #type_ident #ty_generics #where_clause {}
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

    let collect_explicit_calls = field_access.iter().map(|(_, ty)| {
        quote! {
            <#ty as #bundle_>::collect_explicit(__collector__);
        }
    });

    let collect_required_calls = field_access.iter().map(|(_, ty)| {
        quote! {
            <#ty as #bundle_>::collect_required(__collector__);
        }
    });

    let write_explicit_calls = field_access.iter().map(|(ident, ty)| {
        quote! {
            unsafe {
                let __offset__ = ::core::mem::offset_of!(Self, #ident);
                <#ty as #bundle_>::write_explicit(<#owning_ptr_>::take_field(&mut __ptr__, __offset__), __writer__);
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

    let apply_effect_calls = field_access.iter().map(|(ident, ty)| {
        if has_effect {
            quote! {
                unsafe {
                    let __offset__ = ::core::mem::offset_of!(Self, #ident);
                    <#ty as #bundle_>::apply_effect(<#owning_ptr_>::take_field(&mut __ptr__, __offset__), __entity__);
                }
            }
        } else {
            TokenStream::new()
        }
    });

    let write_mut = if !field_access.is_empty() {
        quote! { mut }
    } else {
        TokenStream::new()
    };

    let apply_mut = if !field_access.is_empty() && has_effect {
        quote! { mut }
    } else {
        TokenStream::new()
    };

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        const _: () = {
            #[expect(unsafe_code, reason = "bundle implementation is unsafe.")]
            unsafe impl #impl_generics #bundle_ for #type_ident #ty_generics #where_clause {
                const NEED_APPLY_EFFECT: bool = #has_effect;

                fn collect_explicit(__collector__: &mut #component_collector_) {
                    #(#collect_explicit_calls)*
                }

                fn collect_required(__collector__: &mut #component_collector_) {
                    #(#collect_required_calls)*
                }

                unsafe fn write_explicit(#write_mut __ptr__: #owning_ptr_<'_>, __writer__: &mut #component_writer_) {
                    #(#write_explicit_calls)*
                }

                unsafe fn write_required(__writer__: &mut #component_writer_) {
                    #(#write_required_calls)*
                }

                unsafe fn apply_effect(#apply_mut __ptr__: #owning_ptr_<'_>, __entity__: &mut #entity_owned_) {
                    #(#apply_effect_calls)*
                }
            }
        };
    }
    .into()
}
