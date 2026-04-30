use proc_macro2::TokenStream;
use quote::quote;

use crate::derive_data::ReflectStruct;

// Generate `Reflect::reflect_clone` tokens for struct and tuple-struct.
pub(crate) fn get_struct_clone_impl(info: &ReflectStruct) -> TokenStream {
    use crate::path::fp::{CloneFP, DefaultFP, ResultFP};

    let meta = info.meta();
    let voker_reflect_path = meta.voker_reflect_path();
    let macro_utils_ = crate::path::macro_utils_(voker_reflect_path);
    let reflect_ = crate::path::reflect_(voker_reflect_path);
    let reflect_clone_error_ = crate::path::reflect_clone_error_(voker_reflect_path);
    let type_path_ = crate::path::type_path_(voker_reflect_path);

    if meta.attrs().avail_traits.not_cloneable.is_some() {
        return quote! {
            #[inline]
            fn reflect_clone(&self) -> #ResultFP<#macro_utils_::Box<dyn #reflect_>, #reflect_clone_error_> {
                #ResultFP::Err(#reflect_clone_error_::NotSupport {
                    type_path: <Self as #type_path_>::type_path(),
                })
            }
        };
    }

    if let Some(span) = meta.attrs().avail_traits.clone {
        let reflect_clone = syn::Ident::new("reflect_clone", span);

        quote! {
            #[inline]
            fn #reflect_clone(&self) -> #ResultFP<#macro_utils_::Box<dyn #reflect_>, #reflect_clone_error_> {
                #ResultFP::Ok(#macro_utils_::Box::new(<Self as #CloneFP>::clone(self)))
            }
        }
    } else if meta.attrs().avail_traits.default.is_some() {
        let mut tokens = TokenStream::new();

        for field in info.active_fields() {
            let field_ty = &field.data.ty;
            let member = field.to_member();

            tokens.extend(quote! {
                __new_value__.#member = #macro_utils_::reflect_clone_field::<#field_ty>(&self.#member)?;
            });
        }

        quote! {
            fn reflect_clone(&self) -> #ResultFP<#macro_utils_::Box<dyn #reflect_>, #reflect_clone_error_> {
                let mut __new_value__ = <Self as #DefaultFP>::default();

                #tokens

                #ResultFP::Ok(#macro_utils_::Box::new(__new_value__))
            }
        }
    } else {
        let mut unsupported = false;
        for field in info.fields().iter().filter(|f| f.is_ignore()) {
            if !field.cloneable() {
                unsupported = true;
                break;
            }
        }

        if unsupported {
            return quote! {
                fn reflect_clone(&self) -> #ResultFP<#macro_utils_::Box<dyn #reflect_>, #reflect_clone_error_> {
                    #ResultFP::Err(#reflect_clone_error_::NotSupport {
                        type_path: <Self as #type_path_>::type_path(),
                    })
                }
            };
        }

        // All ignored fields provide clone info -> generate mixed clone code:
        let mut tokens = TokenStream::new();
        for field in info.fields().iter() {
            let field_ty = &field.data.ty;
            let member = field.to_member();

            if field.attrs.ignore.is_some() {
                // use direct Clone for ignored fields (user promised clone via attribute)
                tokens.extend(quote! {
                    #member: <#field_ty as #CloneFP>::clone(&self.#member),
                });
            } else {
                // use reflect-based clone for active fields
                tokens.extend(quote! {
                    #member: #macro_utils_::reflect_clone_field::<#field_ty>(&self.#member)?,
                });
            }
        }

        quote! {
            fn reflect_clone(&self) -> #ResultFP<#macro_utils_::Box<dyn #reflect_>, #reflect_clone_error_> {
                #ResultFP::Ok(#macro_utils_::Box::new(
                    Self {
                        #tokens
                    }
                ))
            }
        }
    }
}
