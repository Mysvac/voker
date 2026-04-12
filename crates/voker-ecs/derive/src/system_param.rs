use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::visit_mut::VisitMut;
use syn::{Data, DeriveInput, Fields, GenericParam};

use crate::utils::{contains_any_idents, field_type_constraint};

fn validate_lifetimes(generics: &syn::Generics) -> syn::Result<()> {
    if generics.lifetimes().count() != 2 {
        return Err(syn::Error::new_spanned(
            &generics.params,
            "`SystemParam` requires exactly two lifetime parameters: `'w` and `'s`.",
        ));
    }

    let has_w = generics.lifetimes().any(|lt| lt.lifetime.ident == "w");
    let has_s = generics.lifetimes().any(|lt| lt.lifetime.ident == "s");

    if !has_w || !has_s {
        return Err(syn::Error::new_spanned(
            &generics.params,
            "`SystemParam` lifetime parameters must be exactly `'w` and `'s`.",
        ));
    }

    Ok(())
}

fn build_item_ty(type_ident: &syn::Ident, generics: &syn::Generics) -> proc_macro2::TokenStream {
    let mut item_generics = generics.clone();

    for param in &mut item_generics.params {
        if let GenericParam::Lifetime(lifetime_param) = param {
            if lifetime_param.lifetime.ident == "w" {
                lifetime_param.lifetime = syn::Lifetime::new("'world", Span::call_site());
            } else if lifetime_param.lifetime.ident == "s" {
                lifetime_param.lifetime = syn::Lifetime::new("'state", Span::call_site());
            }
        }
    }

    let (_, item_ty_g, _) = item_generics.split_for_impl();
    quote! { #type_ident #item_ty_g }
}

fn map_static_lifetimes(ty: &syn::Type) -> syn::Type {
    struct LifetimeToStatic;

    impl VisitMut for LifetimeToStatic {
        fn visit_lifetime_mut(&mut self, lifetime: &mut syn::Lifetime) {
            if lifetime.ident == "w" || lifetime.ident == "s" {
                *lifetime = syn::Lifetime::new("'static", Span::call_site());
            }
        }
    }

    let mut out = ty.clone();
    LifetimeToStatic.visit_type_mut(&mut out);
    out
}

pub(crate) fn impl_derive_system_param(mut ast: DeriveInput) -> TokenStream {
    use crate::path::fp::ResultFP;
    let voker_ecs_path = crate::path::voker_ecs();
    let system_param_ = crate::path::system_param_(&voker_ecs_path);
    let system_param_error_ = crate::path::system_param_error_(&voker_ecs_path);
    let world_ = crate::path::world_(&voker_ecs_path);
    let unsafe_world_ = crate::path::unsafe_world_(&voker_ecs_path);
    let access_table_ = crate::path::access_table_(&voker_ecs_path);
    let tick_ = crate::path::tick_(&voker_ecs_path);
    let system_meta_ = crate::path::system_meta_(&voker_ecs_path);
    let deferred_world_ = crate::path::deferred_world_(&voker_ecs_path);

    let Data::Struct(data) = &ast.data else {
        return syn::Error::new_spanned(
            ast,
            "`SystemParam` can only be derived for structs (named, tuple, or unit).",
        )
        .into_compile_error()
        .into();
    };

    if let Err(err) = validate_lifetimes(&ast.generics) {
        return err.into_compile_error().into();
    }

    let type_ident = ast.ident;
    let field_types: Vec<&syn::Type> = data.fields.iter().map(|f| &f.ty).collect();
    let static_field_types: Vec<syn::Type> =
        field_types.iter().map(|ty| map_static_lifetimes(ty)).collect();
    let idx = (0..field_types.len()).map(syn::Index::from).collect::<Vec<_>>();

    // Only add `SystemParam` constraints for types that containing type generics.
    let type_param_idents: Vec<syn::Ident> = ast
        .generics
        .type_params()
        .map(|type_param| type_param.ident.clone())
        .collect();

    for ty in &static_field_types {
        if contains_any_idents(ty, &type_param_idents) {
            field_type_constraint(&mut ast.generics, ty, &system_param_);
        }
    }

    let item_ty = build_item_ty(&type_ident, &ast.generics);
    let (impl_g, ty_g, where_g) = ast.generics.split_for_impl();

    let fetch_init = match &data.fields {
        Fields::Named(fields) => {
            let names = fields.named.iter().map(|f| f.ident.as_ref().expect("named field"));
            quote! {
                #type_ident { #(
                    #names: unsafe {
                        <#static_field_types as #system_param_>::build_param(world, &mut state.#idx, last_run, this_run)?
                    },
                )* }
            }
        }
        Fields::Unnamed(_) => {
            quote! {
                #type_ident ( #(
                    unsafe {
                        <#static_field_types as #system_param_>::build_param(world, &mut state.#idx, last_run, this_run)?
                    },
                )* )
            }
        }
        Fields::Unit => quote! { #type_ident },
    };

    quote! {
        const _: () = {
            #[expect(unsafe_code, reason = "SystemParam implementation is unsafe")]
            unsafe impl #impl_g #system_param_ for #type_ident #ty_g #where_g {
                type State = ( #( <#static_field_types as #system_param_>::State, )* );
                type Item<'world, 'state> = #item_ty;

                const DEFERRED: bool = false #( || <#static_field_types as #system_param_>::DEFERRED )*;
                const NON_SEND: bool = false #( || <#static_field_types as #system_param_>::NON_SEND )*;
                const EXCLUSIVE: bool = false #( || <#static_field_types as #system_param_>::EXCLUSIVE )*;

                fn init_state(world: &mut #world_) -> Self::State {
                    ( #( <#static_field_types as #system_param_>::init_state(world), )* )
                }

                fn mark_access(table: &mut #access_table_, state: &Self::State) -> bool {
                    true #( && <#static_field_types as #system_param_>::mark_access(table, &state.#idx) )*
                }

                unsafe fn build_param<'__w, '__s>(
                    world: #unsafe_world_<'__w>,
                    state: &'__s mut Self::State,
                    last_run: #tick_,
                    this_run: #tick_,
                ) -> #ResultFP<Self::Item<'__w, '__s>, #system_param_error_> {
                    Ok(#fetch_init)
                }

                fn defer(state: &mut Self::State, system_meta: &#system_meta_, mut world: #deferred_world_) {
                    #( <#static_field_types as #system_param_>::defer(&mut state.#idx, system_meta, world.reborrow()); )*
                }

                fn apply_deferred(state: &mut Self::State, system_meta: &#system_meta_, world: &mut #world_) {
                    #( <#static_field_types as #system_param_>::apply_deferred(&mut state.#idx, system_meta, world); )*
                }
            }
        };
    }
    .into()
}
