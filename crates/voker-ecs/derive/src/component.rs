use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Type, parse_quote};

enum Cloner {
    Copy,
    Clone,
    None,
    Auto,
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
}

fn parse_attributes(attrs: &[syn::Attribute]) -> syn::Result<Attributes> {
    let mut ret = Attributes {
        mutable: false,
        storage: Storage::Dense,
        cloner: Cloner::Auto,
        required: None,
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
                        _ => {
                            return Err(meta.error(concat! {
                                    "unsupported storage type, expected ",
                                    "\"dense\" or \"sparse\".",
                            }));
                        }
                    }
                    Ok(())
                } else if meta.path.is_ident("required") {
                    let value = meta.value()?;
                    ret.required = Some(value.parse()?);
                    Ok(())
                } else if meta.path.is_ident("cloner") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;
                    match lit.value().as_str() {
                        "copy" => ret.cloner = Cloner::Copy,
                        "clone" => ret.cloner = Cloner::Clone,
                        "none" => ret.cloner = Cloner::None,
                        "auto" => ret.cloner = Cloner::Auto,
                        _ => {
                            return Err(meta.error(concat! {
                                "unsupported storage type, expected ",
                                "\"auto\", \"copy\", \"clone\" or \"none\".",
                            }));
                        }
                    }
                    Ok(())
                } else {
                    Err(meta.error(concat! {
                        "unsupported component attribute, expected the following:",
                        "- `cloner = \"auto\"/\"copy\"/\"clone\"/\"none\"`\n",
                        "- `mutable = true/false`\n",
                        "- `storages = \"dense\"/\"sparse\"\n",
                        "- `required = T`, T is a Component or the tuple of Components.\n",
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

    use crate::path::fp::{OptionFP, SendFP, SyncFP};
    let voker_ecs_path = crate::path::voker_ecs();
    let component_ = crate::path::component_(&voker_ecs_path);
    let cloner_ = crate::path::cloner_(&voker_ecs_path);
    let component_storage_ = crate::path::component_storage_(&voker_ecs_path);
    let required_ = crate::path::required_(&voker_ecs_path);
    let macro_utils_ = crate::path::macro_utils_(&voker_ecs_path);

    let mutable_tokens = (!attrs.mutable).then(|| quote! { const MUTABLE: bool = false; });

    let cloner_tokens = match attrs.cloner {
        Cloner::None => quote! {},
        Cloner::Copy => quote! {
            fn cloner() -> #OptionFP<#cloner_> {
                Some(#cloner_::copyable::<Self>())
            }
        },
        Cloner::Clone => quote! {
            fn cloner() -> #OptionFP<#cloner_> {
                Some(#cloner_::clonable::<Self>())
            }
        },
        Cloner::Auto => quote! {
            fn cloner() -> #OptionFP<#cloner_> {
                use #macro_utils_::cloner::*;
                (&&&&__CloneSpec::<Self>::INS).__specialized_cloner()
            }
        },
    };

    let storage_tokens = match attrs.storage {
        Storage::Sparse => {
            Some(quote! { const STORAGE: #component_storage_ = #component_storage_::Sparse; })
        }
        Storage::Dense => None,
    };

    let required_tokens = attrs.required.map(|ty| {
        quote! {
            const REQUIRED: #OptionFP<#required_> = #OptionFP::Some(#required_::from::<#ty>());
        }
    });

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

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        const _: () = {
            impl #impl_generics #component_ for #type_ident #ty_generics #where_clause {
                #mutable_tokens
                #cloner_tokens
                #storage_tokens
                #required_tokens
            }
        };
    }
    .into()
}
