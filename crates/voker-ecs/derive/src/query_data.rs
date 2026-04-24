use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{Data, DeriveInput, Fields, GenericParam, visit_mut::VisitMut};

use crate::utils::{contains_any_idents, field_type_constraint};

struct QueryDataAttrs {
    readonly: bool,
}

fn parse_query_data_attrs(attrs: &[syn::Attribute]) -> syn::Result<QueryDataAttrs> {
    let mut out = QueryDataAttrs { readonly: false };

    for attr in attrs {
        if !attr.path().is_ident("query_data") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("readonly") {
                out.readonly = true;
                Ok(())
            } else {
                Err(meta.error("unsupported query_data option; expected `readonly`."))
            }
        })?;
    }

    Ok(out)
}

fn validate_lifetimes(generics: &syn::Generics) -> syn::Result<()> {
    let lifetimes_len = generics.lifetimes().count();
    if lifetimes_len > 1 {
        return Err(syn::Error::new_spanned(
            generics,
            "`QueryData` only accepts a single lifetime named `'w` (or without any lifetime param).",
        ));
    }

    if lifetimes_len == 1 && !generics.lifetimes().any(|lt| lt.lifetime.ident == "w") {
        return Err(syn::Error::new_spanned(
            &generics.params,
            "`QueryData` accepts at most one lifetime parameter, and it must be `'w`.",
        ));
    }

    Ok(())
}

fn validate_no_mut_reference(data: &syn::DataStruct) -> syn::Result<()> {
    for field in &data.fields {
        let ty = &field.ty;
        if let syn::Type::Reference(reference) = ty
            && reference.mutability.is_some()
        {
            return Err(syn::Error::new_spanned(
                ty,
                "`&mut T` is not supported in `#[derive(QueryData)]`; use `Mut<T>` instead.",
            ));
        }
    }

    Ok(())
}

fn build_item_ty(type_ident: &syn::Ident, generics: &syn::Generics) -> proc_macro2::TokenStream {
    let mut item_generics = generics.clone();

    for param in &mut item_generics.params {
        if let GenericParam::Lifetime(lifetime_param) = param {
            lifetime_param.lifetime = syn::Lifetime::new("'world", Span::call_site());
        }
    }

    let (_, item_ty_g, _) = item_generics.split_for_impl();
    quote! { #type_ident #item_ty_g }
}

fn map_static_lifetimes(ty: &syn::Type) -> syn::Type {
    struct LifetimeToStatic;

    impl VisitMut for LifetimeToStatic {
        fn visit_lifetime_mut(&mut self, lifetime: &mut syn::Lifetime) {
            if lifetime.ident == "w" {
                *lifetime = syn::Lifetime::new("'static", Span::call_site());
            }
        }
    }

    let mut out = ty.clone();
    LifetimeToStatic.visit_type_mut(&mut out);

    out
}

pub(crate) fn impl_derive_query_data(mut ast: DeriveInput) -> TokenStream {
    use crate::path::fp::OptionFP;
    let voker_ecs_path = crate::path::voker_ecs();
    let query_data_ = crate::path::query_data_(&voker_ecs_path);
    let readonly_query_data_ = crate::path::readonly_query_data_(&voker_ecs_path);
    let world_ = crate::path::world_(&voker_ecs_path);
    let unsafe_world_ = crate::path::unsafe_world_(&voker_ecs_path);
    let tick_ = crate::path::tick_(&voker_ecs_path);
    let archetype_ = crate::path::archetype_(&voker_ecs_path);
    let table_ = crate::path::table_(&voker_ecs_path);
    let table_row_ = crate::path::table_row_(&voker_ecs_path);
    let entity_ = crate::path::entity_(&voker_ecs_path);
    let access_param_ = crate::path::access_param_(&voker_ecs_path);
    let filter_param_builder_ = crate::path::filter_param_builder_(&voker_ecs_path);

    let Data::Struct(data) = &ast.data else {
        return syn::Error::new_spanned(
            ast,
            "`QueryData` can only be derived for structs (named, tuple, or unit).",
        )
        .into_compile_error()
        .into();
    };

    let attrs = match parse_query_data_attrs(&ast.attrs) {
        Ok(attrs) => attrs,
        Err(err) => return err.into_compile_error().into(),
    };

    if let Err(err) = validate_lifetimes(&ast.generics) {
        return err.into_compile_error().into();
    }

    if let Err(err) = validate_no_mut_reference(data) {
        return err.into_compile_error().into();
    }

    let type_ident = ast.ident;

    let field_types: Vec<&syn::Type> = data.fields.iter().map(|f| &f.ty).collect();
    let static_field_types: Vec<syn::Type> =
        field_types.iter().map(|ty| map_static_lifetimes(ty)).collect();
    let idx = (0..field_types.len()).map(syn::Index::from).collect::<Vec<_>>();

    // Only add `QueryData` constraints for types that containing type generics.
    let type_param_idents: Vec<syn::Ident> = ast
        .generics
        .type_params()
        .map(|type_param| type_param.ident.clone())
        .collect();

    for ty in &static_field_types {
        if contains_any_idents(ty, &type_param_idents) {
            field_type_constraint(&mut ast.generics, ty, &query_data_);
        }
    }

    let item_ty = build_item_ty(&type_ident, &ast.generics);
    let (impl_g, ty_g, where_g) = ast.generics.split_for_impl();

    let readonly_impl = if attrs.readonly {
        let mut readonly_generics = ast.generics.clone();
        for ty in &static_field_types {
            field_type_constraint(&mut readonly_generics, ty, &readonly_query_data_);
        }
        let (ro_impl_g, ro_ty_g, ro_where_g) = readonly_generics.split_for_impl();
        quote! {
            #[expect(unsafe_code, reason = "ReadonlyQueryData implementation is unsafe")]
            unsafe impl #ro_impl_g #readonly_query_data_ for #type_ident #ro_ty_g #ro_where_g {}
        }
    } else {
        quote! {}
    };

    let fetch_init = match &data.fields {
        Fields::Named(fields) => {
            let names = fields.named.iter().map(|f| f.ident.as_ref().expect("named field"));
            quote! {
                #type_ident {
                    #( #names: {
                        <#static_field_types as #query_data_>::fetch(
                            &state.#idx,
                            &mut cache.#idx,
                            entity,
                            table_row,
                        )?
                    }, )*
                }
            }
        }
        Fields::Unnamed(_) => {
            quote! {
                #type_ident(
                    #( {
                        <#static_field_types as #query_data_>::fetch(
                            &state.#idx,
                            &mut cache.#idx,
                            entity,
                            table_row,
                        )?
                    }, )*
                )
            }
        }
        Fields::Unit => quote! { #type_ident },
    };

    quote! {
        const _: () = {
            extern crate alloc as __alloc; // for Vec

            #[expect(unsafe_code, reason = "QueryData implementation is unsafe")]
            unsafe impl #impl_g #query_data_ for #type_ident #ty_g #where_g {
                type State = ( #( <#static_field_types as #query_data_>::State, )* );
                type Cache<'world> = ( #( <#static_field_types as #query_data_>::Cache<'world>, )* );
                type Item<'world> = #item_ty;

                const COMPONENTS_ARE_DENSE: bool = true #( && <#static_field_types as #query_data_>::COMPONENTS_ARE_DENSE )*;

                fn build_state(world: &mut #world_) -> Self::State {
                    ( #( <#static_field_types as #query_data_>::build_state(world), )* )
                }

                fn fetch_state(world: &#world_) -> #OptionFP<Self::State> {
                    #OptionFP::Some(( #( <#static_field_types as #query_data_>::fetch_state(world)?, )* ))
                }

                unsafe fn build_cache<'__w>(
                    state: &Self::State,
                    world: #unsafe_world_<'__w>,
                    last_run: #tick_,
                    this_run: #tick_,
                ) -> Self::Cache<'__w> {
                    unsafe {
                        ( #( <#static_field_types as #query_data_>::build_cache(&state.#idx, world, last_run, this_run), )* )
                    }
                }

                fn build_filter(state: &Self::State, out: &mut __alloc::vec::Vec<#filter_param_builder_>) {
                    #( <#static_field_types as #query_data_>::build_filter(&state.#idx, out); )*
                }

                fn build_access(state: &Self::State, out: &mut #access_param_) -> bool {
                    true #( && <#static_field_types as #query_data_>::build_access(&state.#idx, out) )*
                }

                unsafe fn set_for_arche<'__w>(
                    state: &Self::State,
                    cache: &mut Self::Cache<'__w>,
                    arche: &'__w #archetype_,
                    table: &'__w #table_,
                ) {
                    unsafe {
                        #( <#static_field_types as #query_data_>::set_for_arche(&state.#idx, &mut cache.#idx, arche, table); )*
                    }
                }

                unsafe fn set_for_table<'__w>(
                    state: &Self::State,
                    cache: &mut Self::Cache<'__w>,
                    table: &'__w #table_,
                ) {
                    unsafe {
                        #( <#static_field_types as #query_data_>::set_for_table(&state.#idx, &mut cache.#idx, table); )*
                    }
                }

                unsafe fn fetch<'__w>(
                    state: &Self::State,
                    cache: &mut Self::Cache<'__w>,
                    entity: #entity_,
                    table_row: #table_row_,
                ) -> Option<Self::Item<'__w>> {
                    unsafe { Some(#fetch_init) }
                }
            }

            #readonly_impl
        };
    }
    .into()
}
