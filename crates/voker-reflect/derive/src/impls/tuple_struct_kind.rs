use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::Ident;

use super::{get_auto_register_impl, get_struct_clone_impl};
use super::{get_common_apply_tokens, impl_trait_typed};
use super::{impl_struct_from_reflect, impl_trait_type_path};
use super::{impl_trait_get_type_meta, impl_trait_reflect};

use crate::derive_data::{FieldAccessors, ReflectMeta, ReflectStruct};

/// Implement full reflect for tuple-struct type.
pub(crate) fn impl_tuple_struct(info: &ReflectStruct) -> TokenStream {
    let meta = info.meta();

    // trait: TypePath
    let type_path_trait_tokens = if meta.attrs().impl_switchs.impl_type_path {
        impl_trait_type_path(meta)
    } else {
        crate::utils::empty()
    };

    // trait: Typed
    let typed_trait_tokens = if meta.attrs().impl_switchs.impl_typed {
        impl_trait_typed(meta, info.type_info_tokens(true), false, false)
    } else {
        crate::utils::empty()
    };

    // trait: TupleStruct
    let tuple_struct_trait_tokens = if meta.attrs().impl_switchs.impl_tuple_struct {
        impl_trait_tuple_struct(info)
    } else {
        crate::utils::empty()
    };

    // trait: Reflect
    let reflect_trait_tokens = if meta.attrs().impl_switchs.impl_reflect {
        let apply_tokens = get_tuple_struct_apply_impl(meta);
        let to_dynamic_tokens = get_tuple_struct_to_dynamic_impl(meta);
        let reflect_clone_tokens = get_struct_clone_impl(info);
        let reflect_eq_tokens = get_tuple_struct_eq_impl(meta);
        let reflect_cmp_tokens = get_tuple_struct_cmp_impl(meta);
        let reflect_hash_tokens = get_tuple_struct_hash_impl(meta);
        let reflect_debug_tokens = get_tuple_struct_debug_impl(meta);

        impl_trait_reflect(
            meta,
            quote!(TupleStruct),
            apply_tokens,
            to_dynamic_tokens,
            reflect_clone_tokens,
            reflect_eq_tokens,
            reflect_cmp_tokens,
            reflect_hash_tokens,
            reflect_debug_tokens,
            false,
        )
    } else {
        crate::utils::empty()
    };

    // trait: GetTypeTraits
    let get_type_meta_tokens = if meta.attrs().impl_switchs.impl_get_type_meta {
        impl_trait_get_type_meta(meta, get_registry_dependencies(info))
    } else {
        crate::utils::empty()
    };

    // trait: FromReflect
    let get_from_reflect_tokens = if meta.attrs().impl_switchs.impl_from_reflect {
        impl_struct_from_reflect(info, true)
    } else {
        crate::utils::empty()
    };

    // featuer: auto_resiter
    let auto_register_tokens = get_auto_register_impl(meta);

    quote! {
        #auto_register_tokens

        #type_path_trait_tokens

        #typed_trait_tokens

        #tuple_struct_trait_tokens

        #reflect_trait_tokens

        #get_type_meta_tokens

        #get_from_reflect_tokens
    }
}

/// Generate `TupleStruct` trait implementation tokens.
fn impl_trait_tuple_struct(info: &ReflectStruct) -> TokenStream {
    use crate::path::fp::OptionFP;
    let meta = info.meta();

    let voker_reflect_path = meta.voker_reflect_path();
    let tuple_struct_ = crate::path::tuple_struct_(voker_reflect_path);
    let reflect_ = crate::path::reflect_(voker_reflect_path);
    let tuple_struct_field_iter_ = crate::path::tuple_struct_field_iter_(voker_reflect_path);
    let dynamic_tuple_struct_ = crate::path::dynamic_tuple_struct_(voker_reflect_path);
    let option_ = OptionFP.to_token_stream();

    let FieldAccessors {
        fields_ref,
        fields_mut,
        field_indices,
        field_count,
    } = FieldAccessors::new(info);

    let real_ident = meta.real_ident();
    let (impl_generics, ty_generics, where_clause) = meta.split_generics(true, false, false);

    quote! {
        impl #impl_generics #tuple_struct_ for #real_ident #ty_generics #where_clause {
            fn field(&self, __index__: usize) -> #OptionFP<&dyn #reflect_> {
                match __index__ {
                    #(#field_indices => #option_::Some(#fields_ref),)*
                    _ => #OptionFP::None,
                }
            }

            fn field_mut(&mut self, __index__: usize) -> #OptionFP<&mut dyn #reflect_> {
                match __index__ {
                    #(#field_indices => #option_::Some(#fields_mut),)*
                    _ => #OptionFP::None,
                }
            }
            #[inline]
            fn field_len(&self) -> usize {
                #field_count
            }
            #[inline]
            fn iter_fields(&self) -> #tuple_struct_field_iter_ {
                #tuple_struct_field_iter_::new(self)
            }

            fn to_dynamic_tuple_struct(&self) -> #dynamic_tuple_struct_ {
                let mut dynamic = #dynamic_tuple_struct_::with_capacity(#tuple_struct_::field_len(self));
                dynamic.set_type_info(#reflect_::represented_type_info(self));
                #(dynamic.extend_boxed(#reflect_::to_dynamic(#fields_ref));)*
                dynamic
            }
        }
    }
}

/// Generate `Reflect::apply` implementation tokens.
fn get_tuple_struct_apply_impl(meta: &ReflectMeta) -> TokenStream {
    use crate::path::fp::ResultFP;

    let voker_reflect_path = meta.voker_reflect_path();
    let reflect_ = crate::path::reflect_(voker_reflect_path);
    let apply_error_ = crate::path::apply_error_(voker_reflect_path);
    let tuple_struct_apply_ = crate::path::tuple_struct_apply_(voker_reflect_path);

    let input_ = Ident::new("__input__", Span::call_site());

    let clone_tokens = get_common_apply_tokens(meta, &input_);

    quote! {
        fn apply(&mut self, #input_: &dyn #reflect_) -> #ResultFP<(), #apply_error_> {
            #clone_tokens

            #tuple_struct_apply_(self, #input_)
        }
    }
}

/// Generate `Reflect::to_dynamic` implementation tokens.
fn get_tuple_struct_to_dynamic_impl(meta: &ReflectMeta) -> TokenStream {
    let voker_reflect_path = meta.voker_reflect_path();
    let macro_utils_ = crate::path::macro_utils_(voker_reflect_path);
    let reflect_ = crate::path::reflect_(voker_reflect_path);
    let tuple_struct_ = crate::path::tuple_struct_(voker_reflect_path);

    quote! {
        #[inline]
        fn to_dynamic(&self) -> #macro_utils_::Box<dyn #reflect_> {
            #macro_utils_::Box::new(<Self as #tuple_struct_>::to_dynamic_tuple_struct(self) )
        }
    }
}

/// Generate `Reflect::reflect_eq` implementation tokens.
fn get_tuple_struct_eq_impl(meta: &ReflectMeta) -> TokenStream {
    use crate::path::fp::{OptionFP, PartialEqFP};
    let voker_reflect_path = meta.voker_reflect_path();
    let reflect_ = crate::path::reflect_(voker_reflect_path);

    if let Some(span) = meta.attrs().avail_traits.eq {
        let reflect_eq = Ident::new("reflect_eq", span);

        quote! {
            fn #reflect_eq(&self, __other__: &dyn #reflect_) -> #OptionFP<bool> {
                if let #OptionFP::Some(__value__) = <dyn #reflect_>::downcast_ref::<Self>(__other__) {
                    return #OptionFP::Some( #PartialEqFP::eq(self, __value__) );
                }
                #OptionFP::Some( false )
            }
        }
    } else {
        let tuple_struct_eq_ = crate::path::tuple_struct_eq_(voker_reflect_path);

        quote! {
            #[inline]
            fn reflect_eq(&self, __other__: &dyn #reflect_) -> #OptionFP<bool> {
                #tuple_struct_eq_(self, __other__)
            }
        }
    }
}

/// Generate `Reflect::reflect_cmp` implementation tokens.
fn get_tuple_struct_cmp_impl(meta: &ReflectMeta) -> TokenStream {
    use crate::path::fp::{OptionFP, PartialOrdFP};
    let voker_reflect_path = meta.voker_reflect_path();
    let reflect_ = crate::path::reflect_(voker_reflect_path);

    if let Some(span) = meta.attrs().avail_traits.cmp {
        let reflect_cmp = Ident::new("reflect_cmp", span);

        quote! {
            fn #reflect_cmp(&self, __other__: &dyn #reflect_) -> #OptionFP<::core::cmp::Ordering> {
                if let #OptionFP::Some(__value__) = <dyn #reflect_>::downcast_ref::<Self>(__other__) {
                    return #PartialOrdFP::partial_cmp(self, __value__);
                }
                #OptionFP::None
            }
        }
    } else {
        let tuple_struct_cmp_ = crate::path::tuple_struct_cmp_(voker_reflect_path);

        quote! {
            #[inline]
            fn reflect_cmp(&self, __other__: &dyn #reflect_) -> #OptionFP<::core::cmp::Ordering> {
                #tuple_struct_cmp_(self, __other__)
            }
        }
    }
}

/// Generate `Reflect::reflect_hash` implementation tokens.
fn get_tuple_struct_hash_impl(meta: &ReflectMeta) -> TokenStream {
    use crate::path::fp::{HashFP, HasherFP, OptionFP};
    let voker_reflect_path = meta.voker_reflect_path();

    if let Some(span) = meta.attrs().avail_traits.hash {
        let reflect_hasher = crate::path::reflect_hasher_(voker_reflect_path);
        let reflect_hash = Ident::new("reflect_hash", span);

        quote! {
            fn #reflect_hash(&self) -> #OptionFP<u64> {
                let mut hasher = #reflect_hasher();
                <Self as #HashFP>::hash(self, &mut hasher);
                #OptionFP::Some(#HasherFP::finish(&hasher))
            }
        }
    } else {
        let tuple_struct_hash_ = crate::path::tuple_struct_hash_(voker_reflect_path);

        quote! {
            #[inline]
            fn reflect_hash(&self) -> #OptionFP<u64> {
                #tuple_struct_hash_(self)
            }
        }
    }
}

/// Generate `Reflect::reflect_debug` implementation tokens.
fn get_tuple_struct_debug_impl(meta: &ReflectMeta) -> TokenStream {
    use crate::path::fp::DebugFP;

    if let Some(span) = meta.attrs().avail_traits.debug {
        let reflect_debug = Ident::new("reflect_debug", span);

        quote! {
            #[inline]
            fn #reflect_debug(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                <Self as #DebugFP>::fmt(self, f)
            }
        }
    } else {
        let tuple_struct_debug_ = crate::path::tuple_struct_debug_(meta.voker_reflect_path());

        quote! {
            #[inline]
            fn reflect_debug(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                #tuple_struct_debug_(self, f)
            }
        }
    }
}

/// Generate partial `GetTypeMeta` implementation tokens.
fn get_registry_dependencies(info: &ReflectStruct) -> TokenStream {
    let voker_reflect_path = info.meta().voker_reflect_path();
    let type_registry_ = crate::path::type_registry_(voker_reflect_path);

    let field_types = info.active_fields().map(|x| &x.data.ty);

    quote! {
        fn register_dependencies(__registry__: &mut #type_registry_) {
            #(#type_registry_::register::<#field_types>(__registry__);)*
        }
    }
}
