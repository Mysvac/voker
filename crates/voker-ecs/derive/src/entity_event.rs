use quote::quote;
use syn::{DeriveInput, parse_quote, spanned::Spanned};

pub(crate) fn impl_derive_entity_event(ast: DeriveInput) -> proc_macro::TokenStream {
    use crate::path::fp::{SendFP, SyncFP};

    let voker_ecs_path = crate::path::voker_ecs();
    let event_ = crate::path::event_(&voker_ecs_path);
    let entity_event_ = crate::path::entity_event_(&voker_ecs_path);
    let entity_event_mut_ = crate::path::entity_event_mut_(&voker_ecs_path);
    let entity_ = crate::path::entity_(&voker_ecs_path);
    let child_of_ = crate::path::child_of_(&voker_ecs_path);
    let entity_trigger_ = crate::path::entity_trigger_(&voker_ecs_path);
    let propagate_entity_trigger_ = crate::path::propagate_entity_trigger_(&voker_ecs_path);

    let mut auto_propagate = false;
    let mut propagate = false;
    let mut traversal: Option<syn::Type> = None;
    let mut trigger: Option<syn::Type> = None;

    for attr in &ast.attrs {
        if attr.path().is_ident("entity_event") {
            let ret = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("trigger") {
                    if trigger.is_some() {
                        Err(meta.error("duplicate attribute `trigger`"))
                    } else {
                        trigger = Some(meta.value()?.parse()?);
                        Ok(())
                    }
                } else if meta.path.is_ident("propagate") {
                    if propagate {
                        Err(meta.error("duplicate attribute `propagate`"))
                    } else {
                        propagate = true;
                        if meta.input.peek(syn::Token![=]) {
                            traversal = Some(meta.value()?.parse()?);
                        }
                        Ok(())
                    }
                } else if meta.path.is_ident("auto_propagate") {
                    if propagate {
                        Err(meta.error("duplicate attribute: `propagate` and `auto_propagate`"))
                    } else {
                        propagate = true;
                        auto_propagate = true;
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

    if trigger.is_some() && propagate {
        return syn::Error::new(
            ast.span(),
            "Cannot define both #[entity_event(trigger)] and #[entity_event(propagate)]",
        )
        .into_compile_error()
        .into();
    }

    let entity_field = match get_event_target_field(&ast) {
        Ok(value) => value,
        Err(err) => return err.into_compile_error().into(),
    };

    let trigger = if let Some(trigger) = trigger {
        quote! {#trigger}
    } else if propagate {
        let traversal = traversal.unwrap_or_else(|| parse_quote! { &'static #child_of_ });
        quote! {#propagate_entity_trigger_<#auto_propagate, Self, #traversal>}
    } else {
        quote! {#entity_trigger_}
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

    let set_entity_event_target_impl = if propagate {
        quote! {
            impl #impl_generics #entity_event_mut_ for #type_ident #type_generics #where_clause {
                fn set_event_target(&mut self, entity: #entity_) {
                    self.#entity_field = entity;
                }
            }
        }
    } else {
        quote! {}
    };

    quote! {
        impl #impl_generics #event_ for #type_ident #type_generics #where_clause {
            type Trigger<'a> = #trigger;
        }

        impl #impl_generics #entity_event_ for #type_ident #type_generics #where_clause {
            fn event_target(&self) -> #entity_ {
                self.#entity_field
            }
        }

        #set_entity_event_target_impl
    }
    .into()
}

fn get_event_target_field(ast: &DeriveInput) -> syn::Result<syn::Member> {
    use syn::{Data, DataStruct, Fields, Index, Member};

    let Data::Struct(DataStruct { fields, .. }) = &ast.data else {
        return Err(syn::Error::new(
            ast.span(),
            "EntityEvent can only be derived for structs.",
        ));
    };

    match fields {
        Fields::Named(fields) => {
            let mut target: Option<Member> = None;
            for field in fields.named.iter() {
                let meet = field.attrs.iter().any(|attr| attr.path().is_ident("event_target"));

                if meet {
                    if target.is_some() {
                        return Err(syn::Error::new_spanned(
                            field,
                            "duplicated #[event_target] field",
                        ));
                    }
                    if let Some(ident) = field.ident.clone() {
                        target = Some(Member::Named(ident));
                    }
                }
            }

            target.ok_or_else(|| {
                syn::Error::new(
                    fields.span(),
                    "EntityEvent derive expected unnamed structs\
                with one field or with a field annotated with #[event_target].",
                )
            })
        }
        Fields::Unnamed(fields) => {
            if fields.unnamed.len() == 1 {
                return Ok(Member::Unnamed(Index::from(0)));
            }

            let mut target: Option<Member> = None;
            for (index, field) in fields.unnamed.iter().enumerate() {
                let meet = field.attrs.iter().any(|attr| attr.path().is_ident("event_target"));
                if meet {
                    if target.is_some() {
                        return Err(syn::Error::new_spanned(field, "duplicated #[event_target]"));
                    }
                    target = Some(Member::Unnamed(Index::from(index)));
                }
            }
            target.ok_or_else(|| {
                syn::Error::new(
                    fields.span(),
                    "EntityEvent derive expected unnamed structs\
                with one field or with a field annotated with #[event_target].",
                )
            })
        }
        Fields::Unit => Err(syn::Error::new(
            fields.span(),
            "EntityEvent derive does not work on unit structs.\
            Your type must have a field to store the `Entity` target,\
            such as `Attack(Entity)` or `Attack { #[event_target] entity: Entity }`.",
        )),
    }
}
