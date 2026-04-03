use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Ident;

use super::{get_auto_register_impl, get_common_apply_tokens};
use super::{get_common_from_reflect_tokens, impl_trait_get_type_meta};
use super::{impl_trait_reflect, impl_trait_type_path, impl_trait_typed};

use crate::derive_data::{EnumVariantFields, ReflectEnum, ReflectMeta, StructField};

/// Implement full reflect for enum type.
pub(crate) fn impl_enum(info: &ReflectEnum) -> TokenStream {
    let meta = info.meta();

    // trait: TypePath
    let type_path_trait_tokens = if meta.attrs().impl_switchs.impl_type_path {
        impl_trait_type_path(meta)
    } else {
        crate::utils::empty()
    };

    // trait: Typed
    let typed_trait_tokens = if meta.attrs().impl_switchs.impl_typed {
        impl_trait_typed(meta, info.type_info_tokens(), false, true)
    } else {
        crate::utils::empty()
    };

    // trait: Enum
    let enum_trait_tokens = if meta.attrs().impl_switchs.impl_enum {
        impl_trait_enum(info)
    } else {
        crate::utils::empty()
    };

    // trait: Reflect
    let reflect_trait_tokens = if meta.attrs().impl_switchs.impl_reflect {
        let apply_tokens = get_enum_apply_impl(info);
        let to_dynamic_tokens = get_enum_to_dynamic_impl(meta);
        let reflect_clone_tokens = get_enum_clone_impl(info);
        let reflect_eq_tokens = get_enum_eq_impl(meta);
        let reflect_cmp_tokens = get_enum_cmp_impl(meta);
        let reflect_hash_tokens = get_enum_hash_impl(meta);
        let reflect_debug_tokens = get_enum_debug_impl(meta);

        impl_trait_reflect(
            meta,
            quote!(Enum),
            apply_tokens,
            to_dynamic_tokens,
            reflect_clone_tokens,
            reflect_eq_tokens,
            reflect_cmp_tokens,
            reflect_hash_tokens,
            reflect_debug_tokens,
            true,
        )
    } else {
        crate::utils::empty()
    };

    // trait: GetTypeMeta
    let get_type_meta_tokens = if meta.attrs().impl_switchs.impl_get_type_meta {
        impl_trait_get_type_meta(meta, get_registry_dependencies(info))
    } else {
        crate::utils::empty()
    };

    // trait: FromReflect
    let from_reflect_trait_tokens = if meta.attrs().impl_switchs.impl_from_reflect {
        impl_enum_from_reflect(info)
    } else {
        crate::utils::empty()
    };

    // featuer: auto_resiter
    let auto_register_tokens = get_auto_register_impl(meta);

    quote! {
        #auto_register_tokens

        #type_path_trait_tokens

        #typed_trait_tokens

        #enum_trait_tokens

        #reflect_trait_tokens

        #get_type_meta_tokens

        #from_reflect_trait_tokens
    }
}

/// Generate `Enum` trait implementation tokens.
fn impl_trait_enum(info: &ReflectEnum) -> TokenStream {
    use crate::path::fp::OptionFP;
    let meta = info.meta();

    let voker_reflect_path = meta.voker_reflect_path();
    let enum_ = crate::path::enum_(voker_reflect_path);
    let reflect_ = crate::path::reflect_(voker_reflect_path);
    let variant_field_iter_ = crate::path::variant_field_iter_(voker_reflect_path);
    let variant_kind_ = crate::path::variant_kind_(voker_reflect_path);

    let ref_name = Ident::new("__name__", Span::call_site());
    let ref_index = Ident::new("__index__", Span::call_site());

    let real_ident = meta.real_ident();
    let (impl_generics, ty_generics, where_clause) = meta.split_generics(true, false, true);
    // ↑ The Reflect implementation of `Enum` depends on 'FromReflect'.

    let mut enum_field = Vec::new();
    let mut enum_field_mut = Vec::new();
    let mut enum_field_at = Vec::new();
    let mut enum_field_at_mut = Vec::new();
    // let mut enum_index_of = Vec::new();
    let mut enum_name_at = Vec::new();
    let mut enum_field_len = Vec::new();
    let mut enum_variant_name = Vec::new();
    let mut enum_variant_index = Vec::new();
    let mut enum_variant_kind = Vec::new();

    for (variant_index, variant) in info.variants().iter().enumerate() {
        let ident = &variant.data.ident;
        let name = ident.to_string();
        let variant_path_ = quote!( Self::#ident );

        let variant_type_ident = match variant.data.fields {
            syn::Fields::Unit => Ident::new("Unit", Span::call_site()),
            syn::Fields::Unnamed(..) => Ident::new("Tuple", Span::call_site()),
            syn::Fields::Named(..) => Ident::new("Struct", Span::call_site()),
        };

        enum_variant_name.push(quote! {
            #variant_path_{..} => #name
        });
        enum_variant_index.push(quote! {
            #variant_path_{..} => #variant_index
        });
        enum_variant_kind.push(quote! {
            #variant_path_{..} => #variant_kind_::#variant_type_ident
        });

        fn process_fields(
            fields: &[StructField],
            mut f: impl FnMut(&StructField) + Sized,
        ) -> usize {
            let mut field_len = 0;
            for field in fields.iter() {
                f(field);
                field_len += 1;
            }
            field_len
        }

        match &variant.fields {
            EnumVariantFields::Unit => {
                enum_field_len.push(quote! {
                    #variant_path_{..} => 0usize
                });
            }
            EnumVariantFields::Unnamed(fields) => {
                let field_len = process_fields(fields, |field: &StructField| {
                    let field_index = field.field_index;

                    let declare_field = syn::Index::from(field_index);

                    enum_field_at.push(quote! {
                        #variant_path_ { #declare_field : __value, .. } if #ref_index == #field_index => #OptionFP::Some(__value)
                    });
                    enum_field_at_mut.push(quote! {
                        #variant_path_ { #declare_field : __value, .. } if #ref_index == #field_index => #OptionFP::Some(__value)
                    });
                });

                enum_field_len.push(quote! {
                    #variant_path_{..} => #field_len
                });
            }
            EnumVariantFields::Named(fields) => {
                let field_len = process_fields(fields, |field: &StructField| {
                    let field_ident = field.data.ident.as_ref().unwrap();
                    let field_name = field_ident.to_string();
                    let field_index = field.field_index;

                    enum_field.push(quote! {
                        #variant_path_{ #field_ident: __value__, .. } if #ref_name == #field_name => #OptionFP::Some(__value__)
                    });
                    enum_field_mut.push(quote! {
                        #variant_path_{ #field_ident: __value__, .. } if #ref_name == #field_name => #OptionFP::Some(__value__)
                    });
                    enum_field_at.push(quote! {
                        #variant_path_{ #field_ident: __value__, .. } if #ref_index == #field_index => #OptionFP::Some(__value__)
                    });
                    enum_field_at_mut.push(quote! {
                        #variant_path_{ #field_ident: __value__, .. } if #ref_index == #field_index => #OptionFP::Some(__value__)
                    });
                    enum_name_at.push(quote! {
                        #variant_path_{ .. } if #ref_index == #field_index => #OptionFP::Some(#field_name)
                    });
                });

                enum_field_len.push(quote! {
                    #variant_path_{..} => #field_len
                });
            }
        };
    }

    quote! {
        impl #impl_generics #enum_ for #real_ident #ty_generics #where_clause {
            fn field(&self, #ref_name: &str) -> #OptionFP<&dyn #reflect_> {
                    match self {
                    #(#enum_field,)*
                    _ => #OptionFP::None,
                }
            }

            fn field_at(&self, #ref_index: usize) -> #OptionFP<&dyn #reflect_> {
                match self {
                    #(#enum_field_at,)*
                    _ => #OptionFP::None,
                }
            }

            fn field_mut(&mut self, #ref_name: &str) -> #OptionFP<&mut dyn #reflect_> {
                    match self {
                    #(#enum_field_mut,)*
                    _ => #OptionFP::None,
                }
            }

            fn field_at_mut(&mut self, #ref_index: usize) -> #OptionFP<&mut dyn #reflect_> {
                match self {
                    #(#enum_field_at_mut,)*
                    _ => #OptionFP::None,
                }
            }

            // fn index_of(&self, #ref_name: &str) -> #OptionFP<usize> {
            //         match self {
            //         #(#enum_index_of,)*
            //         _ => #OptionFP::None,
            //     }
            // }

            fn name_at(&self, #ref_index: usize) -> #OptionFP<&str> {
                    match self {
                    #(#enum_name_at,)*
                    _ => #OptionFP::None,
                }
            }

            fn iter_fields(&self) -> #variant_field_iter_ {
                #variant_field_iter_::new(self)
            }

            fn field_len(&self) -> usize {
                match self {
                    #(#enum_field_len,)*
                    _ => unreachable!(), // Used to handle `#[non_exhaustive]`
                }
            }

            fn variant_name(&self) -> &str {
                match self {
                    #(#enum_variant_name,)*
                    _ => unreachable!(), // Used to handle `#[non_exhaustive]`
                }
            }

            fn variant_index(&self) -> usize {
                match self {
                    #(#enum_variant_index,)*
                    _ => unreachable!(), // Used to handle `#[non_exhaustive]`
                }
            }

            fn variant_kind(&self) -> #variant_kind_ {
                match self {
                    #(#enum_variant_kind,)*
                    _ => unreachable!(), // Used to handle `#[non_exhaustive]`
                }
            }
        }
    }
}

/// Generate `Reflect::apply` implementation tokens.
fn get_enum_apply_impl(info: &ReflectEnum) -> TokenStream {
    use crate::path::fp::ResultFP;

    let meta = info.meta();
    let voker_reflect_path = meta.voker_reflect_path();
    let reflect_ = crate::path::reflect_(voker_reflect_path);
    let enum_ = crate::path::enum_(voker_reflect_path);
    let apply_error_ = crate::path::apply_error_(voker_reflect_path);
    let macro_utils_ = crate::path::macro_utils_(voker_reflect_path);
    let from_reflect_ = crate::path::from_reflect_(voker_reflect_path);
    let enum_apply_ = crate::path::enum_apply_(voker_reflect_path);

    let input_ = Ident::new("__input__", Span::call_site());

    let clone_tokens = get_common_apply_tokens(meta, &input_);

    let from_reflect_tokens = if meta.attrs().impl_switchs.impl_from_reflect {
        // `apply` different enum variants is necessary to use `FromReflect` trait, when `clone` is not available,
        //
        // If choose to apply field by field, the code will be very cumbersome, and each field needs to support 'FromReflect'.
        //
        // Therefore, a full apply is chosen here, and it is required to use the default `FromReflect` implementation.
        // (Non-default impl may result in a dead loop caused by the interdependence between the two.)
        quote! {
            if let Some(__val__) = <Self as #from_reflect_>::from_reflect(#input_) {
                *self = __val__;
                return #ResultFP::Ok(());
            }
        }
    } else {
        crate::utils::empty()
    };

    quote! {
        fn apply(&mut self, #input_: &dyn #reflect_) -> #ResultFP<(), #apply_error_>  {
            #clone_tokens

            if let Some(#input_) = #enum_apply_(self, #input_)? {
                #from_reflect_tokens

                return #ResultFP::Err(
                    #apply_error_::MismatchedVariant {
                        from_variant:#macro_utils_::Cow::Owned(#enum_::variant_path(#input_)),
                        to_variant: #macro_utils_::Cow::Owned(<Self as #enum_>::variant_path(self)),
                    }
                );
            }

            #ResultFP::Ok(())
        }
    }
}

/// Generate `Reflect::to_dynamic` implementation tokens.
fn get_enum_to_dynamic_impl(meta: &ReflectMeta) -> TokenStream {
    let voker_reflect_path = meta.voker_reflect_path();
    let macro_utils_ = crate::path::macro_utils_(voker_reflect_path);
    let reflect_ = crate::path::reflect_(voker_reflect_path);
    let enum_ = crate::path::enum_(voker_reflect_path);

    quote! {
        fn to_dynamic(&self) -> #macro_utils_::Box<dyn #reflect_> {
            #macro_utils_::Box::new(<Self as #enum_>::to_dynamic_enum(self) )
        }
    }
}

/// Generate `Reflect::reflect_clone` implementation tokens.
fn get_enum_clone_impl(info: &ReflectEnum) -> TokenStream {
    use crate::path::fp::{CloneFP, ResultFP};

    let meta = info.meta();
    let voker_reflect_path = meta.voker_reflect_path();
    let macro_utils_ = crate::path::macro_utils_(voker_reflect_path);
    let reflect_ = crate::path::reflect_(voker_reflect_path);
    let reflect_clone_error_ = crate::path::reflect_clone_error_(voker_reflect_path);

    if let Some(span) = meta.attrs().avail_traits.clone {
        let reflect_clone = Ident::new("reflect_clone", span);

        // use `Clone::clone` directly.
        quote! {
            fn #reflect_clone(&self) -> #ResultFP<#macro_utils_::Box<dyn #reflect_>, #reflect_clone_error_> {
                #ResultFP::Ok(#macro_utils_::Box::new(<Self as #CloneFP>::clone(self)))
            }
        }
    } else {
        // fallback, try to clone all fields.
        let mut match_tokens = TokenStream::new();

        for variant in info.variants.iter() {
            let ident = &variant.data.ident;
            let variant_path_ = quote!( Self::#ident );

            match variant.data.fields {
                syn::Fields::Unit => {
                    match_tokens.extend(quote! {
                        #variant_path_ => #ResultFP::Ok(#macro_utils_::Box::new(#variant_path_) as #macro_utils_::Box<dyn #reflect_>),
                    });
                }
                syn::Fields::Named(..) | syn::Fields::Unnamed(..) => {
                    let mut member_tokens = TokenStream::new();
                    let mut clone_tokens = TokenStream::new();
                    for (index, field) in variant.fields().iter().enumerate() {
                        let field_ty = &field.data.ty;
                        let member = field.to_member();
                        let accessor = Ident::new(&format!("__mem_{index}"), Span::call_site());

                        member_tokens.extend(quote! {
                            #member: #accessor,
                        });
                        clone_tokens.extend(quote! {
                            #member: #macro_utils_::reflect_clone_field::<#field_ty>(#accessor)?,
                        });
                    }
                    match_tokens.extend(quote! {
                        #variant_path_{ #member_tokens } => #ResultFP::Ok(
                            #macro_utils_::Box::new(#variant_path_ { #clone_tokens }) as #macro_utils_::Box<dyn #reflect_>
                        ),
                    });
                }
            }
        }

        quote! {
            fn reflect_clone(&self) -> #ResultFP<#macro_utils_::Box<dyn #reflect_>, #reflect_clone_error_> {
                match self {
                    #match_tokens
                    _ => unreachable!(), // handle `#[non_exhaustive]`
                }
            }
        }
    }
}

/// Generate `Reflect::reflect_eq` implementation tokens.
fn get_enum_eq_impl(meta: &ReflectMeta) -> TokenStream {
    use crate::path::fp::{OptionFP, PartialEqFP};
    let voker_reflect_path = meta.voker_reflect_path();
    let reflect_ = crate::path::reflect_(voker_reflect_path);

    if let Some(span) = meta.attrs().avail_traits.eq {
        let reflect_eq = Ident::new("reflect_eq", span);

        quote! {
            #[inline]
            fn #reflect_eq(&self, __other__: &dyn #reflect_) -> #OptionFP<bool> {
                if let #OptionFP::Some(__value__) = <dyn #reflect_>::downcast_ref::<Self>(__other__) {
                    return #OptionFP::Some( #PartialEqFP::eq(self, __value__) );
                }
                #OptionFP::Some( false )
            }
        }
    } else {
        let enum_eq_ = crate::path::enum_eq_(voker_reflect_path);
        quote! {
            #[inline]
            fn reflect_eq(&self, __other__: &dyn #reflect_) -> #OptionFP<bool> {
                #enum_eq_(self, __other__)
            }
        }
    }
}

/// Generate `Reflect::reflect_cmp` implementation tokens.
fn get_enum_cmp_impl(meta: &ReflectMeta) -> TokenStream {
    use crate::path::fp::{OptionFP, PartialOrdFP};
    let voker_reflect_path = meta.voker_reflect_path();
    let reflect_ = crate::path::reflect_(voker_reflect_path);

    if let Some(span) = meta.attrs().avail_traits.cmp {
        let reflect_cmp = Ident::new("reflect_cmp", span);

        quote! {
            #[inline]
            fn #reflect_cmp(&self, __other__: &dyn #reflect_) -> #OptionFP<::core::cmp::Ordering> {
                if let #OptionFP::Some(__value__) = <dyn #reflect_>::downcast_ref::<Self>(__other__) {
                    return #PartialOrdFP::partial_cmp(self, __value__);
                }
                #OptionFP::None
            }
        }
    } else {
        let enum_cmp_ = crate::path::enum_cmp_(voker_reflect_path);
        quote! {
            #[inline]
            fn reflect_cmp(&self, __other__: &dyn #reflect_) -> #OptionFP<::core::cmp::Ordering> {
                #enum_cmp_(self, __other__)
            }
        }
    }
}

/// Generate `Reflect::reflect_hash` implementation tokens.
fn get_enum_hash_impl(meta: &ReflectMeta) -> TokenStream {
    use crate::path::fp::{HashFP, HasherFP, OptionFP};
    let voker_reflect_path = meta.voker_reflect_path();
    let reflect_hasher = crate::path::reflect_hasher_(voker_reflect_path);

    if let Some(span) = meta.attrs().avail_traits.hash {
        let reflect_hash = Ident::new("reflect_hash", span);

        quote! {
            #[inline]
            fn #reflect_hash(&self) -> #OptionFP<u64> {
                let mut hasher = #reflect_hasher();
                <Self as #HashFP>::hash(self, &mut hasher);
                #OptionFP::Some(#HasherFP::finish(&hasher))
            }
        }
    } else {
        let enum_hash_ = crate::path::enum_hash_(voker_reflect_path);
        quote! {
            #[inline]
            fn reflect_hash(&self) -> #OptionFP<u64> {
                #enum_hash_(self)
            }
        }
    }
}

/// Generate `Reflect::reflect_debug` implementation tokens.
fn get_enum_debug_impl(meta: &ReflectMeta) -> TokenStream {
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
        let enum_debug_ = crate::path::enum_debug_(meta.voker_reflect_path());
        quote! {
            #[inline]
            fn reflect_debug(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                #enum_debug_(self, f)
            }
        }
    }
}

/// Generate partial `GetTypeMeta` implementation tokens.
fn get_registry_dependencies(info: &ReflectEnum) -> TokenStream {
    let voker_reflect_path = info.meta().voker_reflect_path();
    let type_registry_ = crate::path::type_registry_(voker_reflect_path);

    let field_types = info.active_fields().map(|x| &x.data.ty);

    quote! {
        fn register_dependencies(__registry__: &mut #type_registry_) {
            #(#type_registry_::register::<#field_types>(__registry__);)*
        }
    }
}

/// Generate `FromReflect` trait implementation tokens.
fn impl_enum_from_reflect(info: &ReflectEnum) -> TokenStream {
    use crate::path::fp::OptionFP;
    let meta = info.meta();

    let voker_reflect_path = meta.voker_reflect_path();
    let reflect_ = crate::path::reflect_(voker_reflect_path);
    let from_reflect_ = crate::path::from_reflect_(voker_reflect_path);
    let reflect_ref_ = crate::path::reflect_ref_(voker_reflect_path);
    let enum_ = crate::path::enum_(voker_reflect_path);

    let input_ = Ident::new("__input__", Span::call_site());

    let clone_tokens = get_common_from_reflect_tokens(meta, &input_);

    // See the `quote!` at the end of the function.
    let mut match_tokens = TokenStream::new();

    for variant in info.variants.iter() {
        let ident = &variant.data.ident;
        let variant_path_ = quote!( Self::#ident );
        let variant_name_ = ident.to_string();

        match variant.data.fields {
            syn::Fields::Unit => {
                match_tokens.extend(quote! {
                    #variant_name_ => { return #OptionFP::Some(#variant_path_); },
                });
            }
            syn::Fields::Named(..) | syn::Fields::Unnamed(..) => {
                let mut clone_tokens = TokenStream::new();

                for field in variant.fields().iter() {
                    let field_ty = &field.data.ty;
                    let member = field.to_member();

                    let getter = match &field.data.ident {
                        Some(id) => {
                            let name = id.to_string();
                            quote! { #enum_::field(#input_, #name)? }
                        }
                        None => {
                            let index = field.field_index;
                            quote! { #enum_::field_at(#input_, #index)? }
                        }
                    };

                    clone_tokens.extend(quote! {
                        #member: <#field_ty as #from_reflect_>::from_reflect(#getter)?,
                    });
                }

                match_tokens.extend(quote! {
                    #variant_name_ => {
                        let __result__ = #variant_path_{ #clone_tokens };
                        return #OptionFP::Some(__result__);
                    },
                });
            }
        }
    }

    let real_ident = meta.real_ident();
    let (impl_generics, ty_generics, where_clause) = meta.split_generics(true, false, true);

    quote! {
        impl #impl_generics #from_reflect_ for #real_ident #ty_generics #where_clause  {
            fn from_reflect(#input_: &dyn #reflect_) -> #OptionFP<Self> {
                #clone_tokens

                if let #reflect_ref_::Enum(#input_) = #reflect_::reflect_ref(#input_) {
                    match #enum_::variant_name(#input_) {
                        #match_tokens
                        _ => {
                            return #OptionFP::None;
                        }
                    }
                }

                #OptionFP::None
            }
        }
    }
}
