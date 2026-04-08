#![expect(clippy::if_same_then_else)]

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Type, parse_quote};

enum Cloner {
    Copy,
    Clone,
}

enum Storage {
    Dense,
    Sparse,
}

struct Attributes {
    mutable: bool,
    cloner: Cloner,
    storage: Storage,
    required: Option<Type>,
    on_add: Option<syn::ExprPath>,
    on_clone: Option<syn::ExprPath>,
    on_insert: Option<syn::ExprPath>,
    on_remove: Option<syn::ExprPath>,
    on_discard: Option<syn::ExprPath>,
    on_despawn: Option<syn::ExprPath>,
}

fn parse_hook_path(meta: &syn::meta::ParseNestedMeta) -> syn::Result<syn::ExprPath> {
    if meta.input.peek(syn::Token![=]) {
        let value = meta.value()?;
        value.parse::<syn::ExprPath>()
    } else {
        let ident = meta.path.get_ident().unwrap();
        let hook_name = ident.to_string();
        syn::parse_str::<syn::ExprPath>(&format!("Self::{}", hook_name))
    }
}

fn parse_attributes(attrs: &[syn::Attribute]) -> syn::Result<Attributes> {
    let mut ret = Attributes {
        mutable: false,
        storage: Storage::Dense,
        cloner: Cloner::Clone,
        required: None,
        on_add: None,
        on_clone: None,
        on_insert: None,
        on_remove: None,
        on_discard: None,
        on_despawn: None,
    };

    for attr in attrs {
        if attr.path().is_ident("component") {
            let result = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("mutable") {
                    let value = meta.value()?;
                    let lit: syn::LitBool = value.parse()?;
                    ret.mutable = lit.value;
                    Ok(())
                } else if meta.path.is_ident("storage") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;
                    match lit.value().as_str() {
                        "sparse" => ret.storage = Storage::Sparse,
                        "dense" => ret.storage = Storage::Dense,
                        _ => return Err(meta.error(
                            "unsupported storage type, expected \"dense\" or \"sparse\".",
                        )),
                    }
                    Ok(())
                } else if meta.path.is_ident("required") {
                    let value = meta.value()?;
                    ret.required = Some(value.parse()?);
                    Ok(())
                } else if meta.path.is_ident("on_add") {
                    ret.on_remove = Some(parse_hook_path(&meta)?);
                    Ok(())
                } else if meta.path.is_ident("on_clone") {
                    ret.on_remove = Some(parse_hook_path(&meta)?);
                    Ok(())
                } else if meta.path.is_ident("on_insert") {
                    ret.on_remove = Some(parse_hook_path(&meta)?);
                    Ok(())
                } else if meta.path.is_ident("on_remove") {
                    ret.on_remove = Some(parse_hook_path(&meta)?);
                    Ok(())
                } else if meta.path.is_ident("on_discard") {
                    ret.on_remove = Some(parse_hook_path(&meta)?);
                    Ok(())
                } else if meta.path.is_ident("on_despawn") {
                    ret.on_remove = Some(parse_hook_path(&meta)?);
                    Ok(())
                } else if meta.path.is_ident("Copy") {
                    ret.cloner = Cloner::Copy;
                    Ok(())
                } else {
                    Err(meta.error(concat! {
                        "unsupported component attribute, expected the following:",
                        "- `Copy`\n",
                        "- `mutable = true/false`\n",
                        "- `storage = \"dense\"/\"sparse\"\n",
                        "- `required = T`, T is a Component or the tuple of Components.\n",
                        "- `on_add = path::to::function` or `on_add` (defaults to Self::on_add)\n",
                        "- `on_clone = path::to::function` or `on_clone (defaults to Self::on_clone)`\n",
                        "- `on_insert = path::to::function` or `on_insert` (defaults to Self::on_insert)`\n",
                        "- `on_remove = path::to::function` or `on_remove` (defaults to Self::on_remove)`\n",
                        "- `on_discard = path::to::function` or `on_discard` (defaults to Self::on_discard)`\n",
                        "- `on_despawn = path::to::function` or `on_despawn` (defaults to Self::on_despawn)`\n",
                    }))
                }
            });
            result?;
        }
    }

    Ok(ret)
}

pub(crate) fn impl_derive_component(ast: DeriveInput) -> TokenStream {
    let attrs = match parse_attributes(&ast.attrs) {
        Ok(a) => a,
        Err(e) => return e.into_compile_error().into(),
    };

    use crate::path::fp::{CloneFP, OptionFP, SendFP, SyncFP};
    let voker_ecs_path = crate::path::voker_ecs();
    let component_ = crate::path::component_(&voker_ecs_path);
    let cloner_ = crate::path::cloner_(&voker_ecs_path);
    let storage_mode_ = crate::path::storage_mode_(&voker_ecs_path);
    let required_ = crate::path::required_(&voker_ecs_path);
    let component_hook_ = crate::path::component_hook_(&voker_ecs_path);

    let mutable_tokens = (!attrs.mutable).then(|| quote! { const MUTABLE: bool = false; });

    let cloner_tokens = match attrs.cloner {
        Cloner::Copy => Some(quote! { const CLONER: #cloner_ = #cloner_::copyable::<Self>(); }),
        Cloner::Clone => None,
    };

    let storage_tokens = match attrs.storage {
        Storage::Sparse => Some(quote! { const STORAGE: #storage_mode_ = #storage_mode_::Sparse; }),
        Storage::Dense => None,
    };

    let required_tokens = attrs.required.map(|ty| {
        quote! {
            const REQUIRED: #OptionFP<#required_> = #OptionFP::Some(#required_::from::<#ty>());
        }
    });

    let on_add_tokens = attrs.on_add.map(|ty| {
        quote! {
            const ON_ADD: #OptionFP::Option<#component_hook_> = #OptionFP::Some(#ty);
        }
    });

    let on_clone_tokens = attrs.on_clone.map(|ty| {
        quote! {
            const ON_CLONE: #OptionFP::Option<#component_hook_> = #OptionFP::Some(#ty);
        }
    });

    let on_insert_tokens = attrs.on_insert.map(|ty| {
        quote! {
            const ON_INSERT: #OptionFP::Option<#component_hook_> = #OptionFP::Some(#ty);
        }
    });

    let on_remove_tokens = attrs.on_remove.map(|ty| {
        quote! {
            const ON_REMOVE: #OptionFP::Option<#component_hook_> = #OptionFP::Some(#ty);
        }
    });

    let on_discard_tokens = attrs.on_discard.map(|ty| {
        quote! {
            const ON_DISCARD: #OptionFP::Option<#component_hook_> = #OptionFP::Some(#ty);
        }
    });

    let on_despawn_tokens = attrs.on_despawn.map(|ty| {
        quote! {
            const ON_DESPAWN: #OptionFP::Option<#component_hook_> = #OptionFP::Some(#ty);
        }
    });

    let type_ident = ast.ident;

    let mut generics = ast.generics;
    if generics.type_params().next().is_some() {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote! { Self: #SendFP + #SyncFP + #CloneFP + Sized + 'static });
    } else if generics.lifetimes().next().is_some() {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote! { Self: 'static });
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        const _: () = {
            impl #impl_generics #component_ for #type_ident #ty_generics #where_clause {
                #mutable_tokens
                #cloner_tokens
                #storage_tokens
                #required_tokens

                #on_add_tokens
                #on_clone_tokens
                #on_insert_tokens
                #on_remove_tokens
                #on_discard_tokens
                #on_despawn_tokens
            }
        };
    }
    .into()
}
