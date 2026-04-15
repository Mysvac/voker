use quote::quote;
use syn::{DeriveInput, Type, parse_quote};

pub(crate) fn impl_derive_event(ast: DeriveInput) -> proc_macro::TokenStream {
    use crate::path::fp::{SendFP, SyncFP};
    let voker_ecs_path = crate::path::voker_ecs();
    let event_ = crate::path::event_(&voker_ecs_path);
    let global_trigger_ = crate::path::global_trigger_(&voker_ecs_path);

    let mut trigger: Option<Type> = None;

    for attr in &ast.attrs {
        if attr.path().is_ident("event") {
            let ret = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("trigger") {
                    if trigger.is_some() {
                        Err(meta.error("duplicate attribute"))
                    } else {
                        trigger = Some(meta.value()?.parse()?);
                        Ok(())
                    }
                } else {
                    Err(meta.error("Unsupported attribute"))
                }
            });

            if let Err(e) = ret {
                return e.to_compile_error().into();
            }
        }
    }

    let trigger = if let Some(trigger) = trigger {
        quote! {#trigger}
    } else {
        quote! {#global_trigger_}
    };

    let type_ident = ast.ident;
    let mut generics = ast.generics;
    if generics.type_params().next().is_some() {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote! { Self: #SendFP + #SyncFP + 'static });
    } else if generics.lifetimes().next().is_some() {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote! { Self: 'static });
    }

    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    quote! {
        impl #impl_generics #event_ for #type_ident #type_generics #where_clause {
            type Trigger<'a> = #trigger;
        }
    }
    .into()
}
