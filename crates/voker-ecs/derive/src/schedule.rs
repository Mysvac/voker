use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_quote};

#[derive(Default)]
struct SystemSetAttrs {
    typed: bool,
}

fn parse_system_set_attrs(attrs: &[syn::Attribute]) -> syn::Result<SystemSetAttrs> {
    let mut parsed = SystemSetAttrs::default();

    for attr in attrs {
        if !attr.path().is_ident("system_set") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("typed") {
                parsed.typed = true;
                return Ok(());
            }

            Err(meta.error("unsupported #[system_set(...)] option"))
        })?;
    }

    Ok(parsed)
}

pub(crate) fn impl_derive_schedule_label(ast: DeriveInput) -> TokenStream {
    let voker_ecs_path = crate::path::voker_ecs();
    let trait_path = crate::path::schedule_label_(&voker_ecs_path);

    voker_macro_utils::derive_label(ast, trait_path)
}

pub(crate) fn impl_derive_system_set(ast: DeriveInput) -> TokenStream {
    use crate::path::fp::{CloneFP, DebugFP, EqFP, HashFP, SendFP, SyncFP};

    let attrs = match parse_system_set_attrs(&ast.attrs) {
        Ok(attrs) => attrs,
        Err(err) => return err.into_compile_error().into(),
    };

    let voker_ecs_path = crate::path::voker_ecs();
    let system_set_ = crate::path::system_set_(&voker_ecs_path);
    let system_ = crate::path::system_(&voker_ecs_path);
    let into_system_ = crate::path::into_system_(&voker_ecs_path);
    let system_set_begin_ = crate::path::system_set_begin_(&voker_ecs_path);
    let system_set_end_ = crate::path::system_set_end_(&voker_ecs_path);

    let type_ident = ast.ident;

    let mut generics = ast.generics;
    if generics.type_params().next().is_some() {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote! { Self: #SendFP + #SyncFP + #CloneFP + #DebugFP + #HashFP + #EqFP + 'static });
    } else if generics.lifetimes().next().is_some() {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote! { Self: 'static });
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let (begin_body, end_body) = if attrs.typed {
        match &ast.data {
            Data::Struct(_) | Data::Enum(_) => {}
            _ => {
                return syn::Error::new_spanned(
                    &type_ident,
                    "SystemSet derive only supports structs or enums",
                )
                .to_compile_error()
                .into();
            }
        }

        (
            quote! {
                __alloc::boxed::Box::new(#into_system_::into_system(<#system_set_begin_<Self, 0>>::new()))
            },
            quote! {
                __alloc::boxed::Box::new(#into_system_::into_system(<#system_set_end_<Self, 0>>::new()))
            },
        )
    } else {
        let begin_body = match &ast.data {
            Data::Struct(item) => {
                if !matches!(item.fields, Fields::Unit) {
                    return syn::Error::new_spanned(
                        &item.fields,
                        "SystemSet derive only supports unit structs or enums \
                        with unit variants; use #[system_set(typed)] to enable typed mode",
                    )
                    .to_compile_error()
                    .into();
                }

                quote! {
                    __alloc::boxed::Box::new(#into_system_::into_system(<#system_set_begin_<Self, 0>>::new()))
                }
            }
            Data::Enum(item) => {
                let mut arms = Vec::with_capacity(item.variants.len());

                for (index, variant) in item.variants.iter().enumerate() {
                    if !matches!(variant.fields, Fields::Unit) {
                        return syn::Error::new_spanned(
                            variant,
                            "SystemSet derive for enums only supports unit variants; \
                            use #[system_set(typed)] to enable typed mode",
                        )
                        .to_compile_error()
                        .into();
                    }

                    let variant_ident = &variant.ident;
                    let tag = index;
                    arms.push(quote! {
                        Self::#variant_ident => {
                            __alloc::boxed::Box::new(#into_system_::into_system(<#system_set_begin_<Self, #tag>>::new()))
                        }
                    });
                }

                quote! {
                    match self {
                        #(#arms),*
                    }
                }
            }
            _ => {
                return syn::Error::new_spanned(
                    &type_ident,
                    "SystemSet derive only supports unit structs or enums with unit variants; use #[system_set(typed)] to enable typed mode",
                )
                .to_compile_error()
                .into();
            }
        };

        let end_body = match &ast.data {
            Data::Struct(_) => {
                quote! {
                    __alloc::boxed::Box::new(#into_system_::into_system(<#system_set_end_<Self, 0>>::new()))
                }
            }
            Data::Enum(item) => {
                let mut arms = Vec::with_capacity(item.variants.len());

                for (index, variant) in item.variants.iter().enumerate() {
                    let variant_ident = &variant.ident;
                    let tag = index;
                    arms.push(quote! {
                        Self::#variant_ident => {
                            __alloc::boxed::Box::new(#into_system_::into_system(<#system_set_end_<Self, #tag>>::new()))
                        }
                    });
                }

                quote! {
                    match self {
                        #(#arms),*
                    }
                }
            }
            _ => unreachable!(),
        };

        (begin_body, end_body)
    };

    quote! {
        const _:() = {
            extern crate alloc as __alloc; // for Box and Vec

            impl #impl_generics #system_set_ for #type_ident #ty_generics #where_clause {
                fn begin(&self) -> __alloc::boxed::Box<dyn #system_<Input = (), Output = ()>> {
                    #begin_body
                }

                fn end(&self) -> __alloc::boxed::Box<dyn #system_<Input = (), Output = ()>> {
                    #end_body
                }

                fn dyn_clone(&self) -> __alloc::boxed::Box<dyn #system_set_> {
                    __alloc::boxed::Box::new(#CloneFP::clone(self))
                }
            }
        };
    }
    .into()
}
