use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Ident, spanned::Spanned};

use crate::derive_data::ReflectMeta;

/// Generate implementation code for `GetTypeMeta` trait.
///
/// `register_deps_tokens` is usually related to the type of field.
///
/// For param `add_from_reflect`, See [`ReflectMeta::split_generics`]
pub(crate) fn impl_trait_get_type_meta(
    meta: &ReflectMeta,
    register_deps_tokens: TokenStream,
) -> TokenStream {
    let voker_reflect_path = meta.voker_reflect_path();
    let get_type_meta_ = crate::path::get_type_meta_(voker_reflect_path);
    let type_meta_ = crate::path::type_meta_(voker_reflect_path);
    let from_type_ = crate::path::from_type_(voker_reflect_path);
    let type_data_from_ptr = crate::path::reflect_from_ptr_(voker_reflect_path);

    let voker_ecs_path = if meta.attrs().avail_traits.from_world.is_some()
        || meta.attrs().avail_traits.component.is_some()
        || meta.attrs().avail_traits.resource.is_some()
    {
        Some(crate::path::voker_ecs())
    } else {
        None
    };

    let outer_ = Ident::new("__ret__", Span::call_site());

    // `1` : ReflectFromPtr
    let mut data_counter = 1usize;

    // We can only add `ReflectFromReflect` when using the default `FromReflect` implementation.
    // If it is uniformly added, there may be issues with mismatched generic constraints.
    let insert_from_reflect = if meta.attrs().impl_switchs.impl_from_reflect {
        data_counter += 1;
        let reflect_from_reflect_ = crate::path::reflect_from_reflect_(voker_reflect_path);

        quote! {
            #type_meta_::insert_data::<#reflect_from_reflect_>(&mut #outer_, #from_type_::<Self>::from_type());
        }
    } else {
        crate::utils::empty()
    };

    let insert_default = match meta.attrs().avail_traits.default {
        Some(span) => {
            data_counter += 1;
            let reflect_default_ = crate::path::reflect_default_(voker_reflect_path);

            let from_type_fn = Ident::new("from_type", span);

            quote! {
                #type_meta_::insert_data::<#reflect_default_>(&mut #outer_, #from_type_::<Self>::#from_type_fn());
            }
        }
        None => crate::utils::empty(),
    };

    let insert_serialize = match meta.attrs().avail_traits.serialize {
        Some(span) => {
            data_counter += 1;
            let reflect_serialize_ = crate::path::reflect_serialize_(voker_reflect_path);
            let from_type_fn = Ident::new("from_type", span);

            quote! {
                #type_meta_::insert_data::<#reflect_serialize_>(&mut #outer_, #from_type_::<Self>::#from_type_fn());
            }
        }
        None => crate::utils::empty(),
    };

    let insert_deserialize = match meta.attrs().avail_traits.deserialize {
        Some(span) => {
            data_counter += 1;
            let reflect_deserialize_ = crate::path::reflect_deserialize_(voker_reflect_path);
            let from_type_fn = Ident::new("from_type", span);

            quote! {
                #type_meta_::insert_data::<#reflect_deserialize_>(&mut #outer_, #from_type_::<Self>::#from_type_fn());
            }
        }
        None => crate::utils::empty(),
    };

    let insert_from_world = match meta.attrs().avail_traits.from_world {
        Some(span) => {
            data_counter += 1;
            let from_type_fn = Ident::new("from_type", span);
            quote! {
                #type_meta_::insert_data::<#voker_ecs_path::reflect::ReflectFromWorld>(&mut #outer_, #from_type_::<Self>::#from_type_fn());
            }
        }
        None => crate::utils::empty(),
    };

    let insert_component = match meta.attrs().avail_traits.component {
        Some(span) => {
            data_counter += 1;
            let from_type_fn = Ident::new("from_type", span);
            quote! {
                #type_meta_::insert_data::<#voker_ecs_path::reflect::ReflectComponent>(&mut #outer_, #from_type_::<Self>::#from_type_fn());
            }
        }
        None => crate::utils::empty(),
    };

    let insert_resource = match meta.attrs().avail_traits.resource {
        Some(span) => {
            data_counter += 1;
            let from_type_fn = Ident::new("from_type", span);
            quote! {
                #type_meta_::insert_data::<#voker_ecs_path::reflect::ReflectResource>(&mut #outer_, #from_type_::<Self>::#from_type_fn());
            }
        }
        None => crate::utils::empty(),
    };

    data_counter += meta.attrs().extra_type_data.len();

    let insert_extra_traits = meta.attrs().extra_type_data.iter().map(|extra_path| {
        let span = extra_path.span();
        let from_type_fn = Ident::new("from_type", span);

        quote! {
            #type_meta_::insert_data::<#extra_path>(&mut #outer_, #from_type_::<Self>::#from_type_fn());
        }
    });

    let real_ident = meta.real_ident();
    let (impl_generics, ty_generics, where_clause) = meta.split_generics(true, true, true);

    quote! {
        impl #impl_generics #get_type_meta_ for #real_ident #ty_generics #where_clause {
            fn get_type_meta() -> #type_meta_ {
                let mut #outer_ = #type_meta_::with_capacity::<Self>(#data_counter);
                #type_meta_::insert_data::<#type_data_from_ptr>(&mut #outer_, #from_type_::<Self>::from_type());
                #insert_from_reflect
                #insert_default
                #insert_serialize
                #insert_deserialize
                #insert_from_world
                #insert_component
                #insert_resource
                #(#insert_extra_traits)*
                #outer_
            }

            #register_deps_tokens
        }
    }
}
