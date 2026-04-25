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

    let outer_ = Ident::new("__ret__", Span::call_site());

    let mut data_counter = 0usize;

    // We can only add `ReflectFromReflect` when using the default `FromReflect` implementation.
    // If it is uniformly added, there may be issues with mismatched generic constraints.
    let insert_from_reflect = if meta.attrs().impl_switchs.impl_from_reflect {
        data_counter += 1;
        let reflect_from_reflect_ = crate::path::reflect_from_reflect_(voker_reflect_path);

        quote! {
            #type_meta_::insert_data::<#reflect_from_reflect_>(&mut #outer_, #from_type_::<Self>::from_type());
        }
    } else {
        TokenStream::new()
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
        None => TokenStream::new(),
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
        None => TokenStream::new(),
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
        None => TokenStream::new(),
    };

    let insert_convert = if meta.attrs().into_types.is_empty() && meta.attrs().from_types.is_empty()
    {
        TokenStream::new()
    } else {
        data_counter += 1;
        let reflect_convert_ = crate::path::reflect_convert_(voker_reflect_path);
        let into_types = &meta.attrs().into_types;
        let from_types = &meta.attrs().from_types;

        quote! {
            let mut __reflect_converter = #reflect_convert_::new::<Self>();
            #( __reflect_converter.register_into::<Self, #into_types>(); )*
            #( __reflect_converter.register_from::<Self, #from_types>(); )*
            #type_meta_::insert_data::<#reflect_convert_>(&mut #outer_, __reflect_converter);
        }
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
                #insert_from_reflect
                #insert_default
                #insert_serialize
                #insert_deserialize
                #insert_convert
                #(#insert_extra_traits)*
                #outer_
            }

            #register_deps_tokens
        }
    }
}
