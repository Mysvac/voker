use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_quote};

#[derive(PartialEq, Eq)]
enum Cloner {
    None,
    Clone,
    Copy,
}

struct Attributes {
    mutable: bool,
    cloner: Cloner,
}

fn parse_attributes(attrs: &[syn::Attribute]) -> syn::Result<Attributes> {
    let mut ret = Attributes {
        mutable: false,
        cloner: Cloner::None,
    };

    for attr in attrs {
        if attr.path().is_ident("resource") {
            let result = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("mutable") {
                    let value = meta.value()?;
                    let lit: syn::LitBool = value.parse()?;
                    ret.mutable = lit.value;
                    Ok(())
                } else if meta.path.is_ident("Clone") {
                    if ret.cloner != Cloner::Copy {
                        ret.cloner = Cloner::Clone;
                    }
                    Ok(())
                } else if meta.path.is_ident("Copy") {
                    ret.cloner = Cloner::Copy;
                    Ok(())
                } else {
                    Err(meta.error(concat! {
                        "unsupported resource attribute, expected the following:",
                        "- `Copy` \n",
                        "- `Clone` \n",
                        "- `mutable = true/false` \n",
                    }))
                }
            });
            result?;
        }
    }

    Ok(ret)
}

pub(crate) fn impl_derive_resource(ast: DeriveInput) -> TokenStream {
    let attrs = match parse_attributes(&ast.attrs) {
        Ok(a) => a,
        Err(e) => return e.into_compile_error().into(),
    };

    use crate::path::fp::OptionFP;
    let voker_ecs_path = crate::path::voker_ecs();
    let resource_ = crate::path::resource_(&voker_ecs_path);
    let cloner_ = crate::path::cloner_(&voker_ecs_path);

    let mutable_tokens = (!attrs.mutable).then(|| quote! { const MUTABLE: bool = false; });

    let cloner_tokens = match attrs.cloner {
        Cloner::Clone => Some(
            quote! { const CLONER: #OptionFP<#cloner_> = #OptionFP::Some(#cloner_::clonable::<Self>()); },
        ),
        Cloner::Copy => Some(
            quote! { const CLONER: #OptionFP<#cloner_> = #OptionFP::Some(#cloner_::copyable::<Self>()); },
        ),
        Cloner::None => None,
    };

    let type_ident = ast.ident;
    let mut generics = ast.generics.clone();
    if generics.type_params().next().is_some() {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote! { Self: Sized + 'static });
    } else if generics.lifetimes().next().is_some() {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote! { Self: 'static });
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        const _:() = {
            impl #impl_generics #resource_ for #type_ident #ty_generics #where_clause {
                #mutable_tokens
                #cloner_tokens
            }
        };
    }
    .into()
}
