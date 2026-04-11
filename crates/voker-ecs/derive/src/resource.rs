use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_quote};

struct Attributes {
    mutable: bool,
}

fn parse_attributes(attrs: &[syn::Attribute]) -> syn::Result<Attributes> {
    let mut ret = Attributes { mutable: false };

    for attr in attrs {
        if attr.path().is_ident("resource") {
            let result = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("mutable") {
                    let value = meta.value()?;
                    let lit: syn::LitBool = value.parse()?;
                    ret.mutable = lit.value;
                    Ok(())
                } else {
                    Err(meta.error("Unsupported attribute"))
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

    let voker_ecs_path = crate::path::voker_ecs();
    let resource_ = crate::path::resource_(&voker_ecs_path);

    let mutable_tokens = (!attrs.mutable).then(|| quote! { const MUTABLE: bool = false; });

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
            }
        };
    }
    .into()
}
