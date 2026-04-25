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

    let type_ident = ast.ident.clone();
    let vis = ast.vis.clone();

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

    let has_lifetime = ast.generics.lifetimes().count() > 0;
    let has_fields = !data.fields.is_empty();

    // Determine whether we generate a companion ReadOnly struct or just use Self.
    // A companion ReadOnly struct is generated for non-readonly structs that have
    // a 'w lifetime (meaning they can hold mutable borrows) and at least one field.
    let generate_readonly_struct = !attrs.readonly && has_lifetime && has_fields;

    // Build the where clause augmented with ReadOnly constraints (used for the
    // companion struct definition and its QueryData impl).
    let readonly_extra_bounds: Vec<proc_macro2::TokenStream> = static_field_types
        .iter()
        .map(|sft| quote! { <#sft as #query_data_>::ReadOnly: #query_data_ })
        .collect();
    let readonly_where_g: proc_macro2::TokenStream = {
        if let Some(wc) = &ast.generics.where_clause {
            let predicates = &wc.predicates;
            quote! { where #predicates #(, #readonly_extra_bounds)* }
        } else if readonly_extra_bounds.is_empty() {
            quote! {}
        } else {
            quote! { where #(#readonly_extra_bounds),* }
        }
    };

    // Compute the ReadOnly type used as `type ReadOnly = ...` in the original impl.
    let (readonly_type, readonly_struct_def, readonly_impls_in_const) = if generate_readonly_struct
    {
        // ----------------------------------------------------------------
        // Non-readonly struct with 'w: generate a companion ReadOnly struct.
        // ----------------------------------------------------------------
        let readonly_ident = syn::Ident::new(&format!("{}ReadOnly", type_ident), type_ident.span());

        // Field types in the ReadOnly struct: delegate each field through its
        // QueryData::ReadOnly associated type to get the read-only item type.
        let readonly_field_tys: Vec<proc_macro2::TokenStream> = static_field_types
            .iter()
            .map(|sft| {
                quote! {
                    <<#sft as #query_data_>::ReadOnly as #query_data_>::Item<'w>
                }
            })
            .collect();

        // ReadOnly item type (used as Item<'world> in the companion's QueryData impl).
        let readonly_item_ty = build_item_ty(&readonly_ident, &ast.generics);

        // Type path for the companion struct, e.g. `FooReadOnly<'static, T>`.
        // 'w is replaced by 'static (sentinel); other params are kept as-is.
        let type_param_idents_refs: Vec<&syn::Ident> = type_param_idents.iter().collect();
        let readonly_static_ty = quote! {
            #readonly_ident<'static #(, #type_param_idents_refs)*>
        };

        // Build the companion struct definition (emitted at module level so its
        // fields are publicly accessible, e.g. for `for item in ro_query { item.field }`).
        let struct_def = match &data.fields {
            Fields::Named(fields) => {
                let names: Vec<_> = fields
                    .named
                    .iter()
                    .map(|f| f.ident.as_ref().expect("named field"))
                    .collect();
                quote! {
                    #[doc(hidden)]
                    #vis struct #readonly_ident #ty_g #readonly_where_g {
                        #( pub #names: #readonly_field_tys, )*
                        #[doc(hidden)]
                        pub __phantom: ::core::marker::PhantomData<&'w ()>,
                    }
                }
            }
            Fields::Unnamed(_) => {
                quote! {
                    #[doc(hidden)]
                    #vis struct #readonly_ident #ty_g #readonly_where_g (
                        #( pub #readonly_field_tys, )*
                        #[doc(hidden)]
                        pub ::core::marker::PhantomData<&'w ()>,
                    );
                }
            }
            Fields::Unit => {
                unreachable!("unit structs are handled by generate_readonly_struct=false")
            }
        };

        // Build the fetch initialiser for the companion struct.
        let readonly_delegate_tys: Vec<proc_macro2::TokenStream> = static_field_types
            .iter()
            .map(|sft| quote! { <#sft as #query_data_>::ReadOnly })
            .collect();

        let readonly_fetch_init = match &data.fields {
            Fields::Named(fields) => {
                let names: Vec<_> = fields
                    .named
                    .iter()
                    .map(|f| f.ident.as_ref().expect("named field"))
                    .collect();
                quote! {
                    #readonly_ident {
                        #( #names: {
                            <#readonly_delegate_tys as #query_data_>::fetch(
                                &state.#idx,
                                &mut cache.#idx,
                                entity,
                                table_row,
                            )?
                        }, )*
                        __phantom: ::core::marker::PhantomData,
                    }
                }
            }
            Fields::Unnamed(_) => {
                quote! {
                    #readonly_ident(
                        #( {
                            <#readonly_delegate_tys as #query_data_>::fetch(
                                &state.#idx,
                                &mut cache.#idx,
                                entity,
                                table_row,
                            )?
                        }, )*
                        ::core::marker::PhantomData,
                    )
                }
            }
            Fields::Unit => unreachable!(),
        };

        // Build the impls for the companion struct (emitted inside `const _`).
        let impls = quote! {
            #[expect(unsafe_code, reason = "ReadOnlyQueryData implementation is unsafe")]
            unsafe impl #impl_g #readonly_query_data_ for #readonly_ident #ty_g #readonly_where_g {}

            #[expect(unsafe_code, reason = "QueryData implementation is unsafe")]
            unsafe impl #impl_g #query_data_ for #readonly_ident #ty_g #readonly_where_g {
                type ReadOnly = Self;
                // Use the original field States (not the ReadOnly::State aliases) so
                // Rust can trivially verify State = <Original as QueryData>::State.
                type State = ( #( <#static_field_types as #query_data_>::State, )* );
                type Cache<'world> = ( #( <<#static_field_types as #query_data_>::ReadOnly as #query_data_>::Cache<'world>, )* );
                type Item<'world> = #readonly_item_ty;

                const COMPONENTS_ARE_DENSE: bool =
                    true #( && <<#static_field_types as #query_data_>::ReadOnly as #query_data_>::COMPONENTS_ARE_DENSE )*;

                fn build_state(world: &mut #world_) -> Self::State {
                    ( #( <#readonly_delegate_tys as #query_data_>::build_state(world), )* )
                }

                fn try_build_state(world: &#world_) -> #OptionFP<Self::State> {
                    #OptionFP::Some(( #( <#readonly_delegate_tys as #query_data_>::try_build_state(world)?, )* ))
                }

                unsafe fn build_cache<'__w>(
                    state: &Self::State,
                    world: #unsafe_world_<'__w>,
                    last_run: #tick_,
                    this_run: #tick_,
                ) -> Self::Cache<'__w> {
                    unsafe {
                        ( #( <#readonly_delegate_tys as #query_data_>::build_cache(&state.#idx, world, last_run, this_run), )* )
                    }
                }

                fn build_filter(state: &Self::State, out: &mut __alloc::vec::Vec<#filter_param_builder_>) {
                    #( <#readonly_delegate_tys as #query_data_>::build_filter(&state.#idx, out); )*
                }

                fn build_access(state: &Self::State, out: &mut #access_param_) -> bool {
                    let valid = true;
                    #( let valid = valid && <#readonly_delegate_tys as #query_data_>::build_access(&state.#idx, out); )*
                    valid
                }

                unsafe fn set_for_arche<'__w>(
                    state: &Self::State,
                    cache: &mut Self::Cache<'__w>,
                    arche: &'__w #archetype_,
                    table: &'__w #table_,
                ) {
                    unsafe {
                        #( <#readonly_delegate_tys as #query_data_>::set_for_arche(&state.#idx, &mut cache.#idx, arche, table); )*
                    }
                }

                unsafe fn set_for_table<'__w>(
                    state: &Self::State,
                    cache: &mut Self::Cache<'__w>,
                    table: &'__w #table_,
                ) {
                    unsafe {
                        #( <#readonly_delegate_tys as #query_data_>::set_for_table(&state.#idx, &mut cache.#idx, table); )*
                    }
                }

                unsafe fn fetch<'__w>(
                    state: &Self::State,
                    cache: &mut Self::Cache<'__w>,
                    entity: #entity_,
                    table_row: #table_row_,
                ) -> Option<Self::Item<'__w>> {
                    unsafe { Some(#readonly_fetch_init) }
                }
            }
        };

        (quote! { #readonly_static_ty }, struct_def, impls)
    } else {
        // ----------------------------------------------------------------
        // Readonly / unit / no-'w: ReadOnly = Self, implement ReadOnlyQueryData.
        // ----------------------------------------------------------------
        let mut ro_generics = ast.generics.clone();
        for ty in &static_field_types {
            field_type_constraint(&mut ro_generics, ty, &readonly_query_data_);
        }
        let (ro_impl_g, ro_ty_g, ro_where_g) = ro_generics.split_for_impl();
        let ro_impl = quote! {
            #[expect(unsafe_code, reason = "ReadOnlyQueryData implementation is unsafe")]
            unsafe impl #ro_impl_g #readonly_query_data_ for #type_ident #ro_ty_g #ro_where_g {}
        };

        (quote! { Self }, quote! {}, ro_impl)
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
        #readonly_struct_def

        const _: () = {
            extern crate alloc as __alloc; // for Vec

            #readonly_impls_in_const

            #[expect(unsafe_code, reason = "QueryData implementation is unsafe")]
            unsafe impl #impl_g #query_data_ for #type_ident #ty_g #where_g {
                type ReadOnly = #readonly_type;
                type State = ( #( <#static_field_types as #query_data_>::State, )* );
                type Cache<'world> = ( #( <#static_field_types as #query_data_>::Cache<'world>, )* );
                type Item<'world> = #item_ty;

                const COMPONENTS_ARE_DENSE: bool = true #( && <#static_field_types as #query_data_>::COMPONENTS_ARE_DENSE )*;

                fn build_state(world: &mut #world_) -> Self::State {
                    ( #( <#static_field_types as #query_data_>::build_state(world), )* )
                }

                fn try_build_state(world: &#world_) -> #OptionFP<Self::State> {
                    #OptionFP::Some(( #( <#static_field_types as #query_data_>::try_build_state(world)?, )* ))
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
                    let valid = true;
                    #( let valid = valid && <#static_field_types as #query_data_>::build_access(&state.#idx, out); )*
                    valid // We should not return early, in order to output a complete error message.
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
        };
    }
    .into()
}
