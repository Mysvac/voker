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
        let mut tokens = TokenStream::new();

        for field in info.fields().iter() {
            let field_ty = &field.data.ty;
            let member = field.to_member();

            tokens.extend(quote! {
                #member: #macro_utils_::reflect_clone_field::<#field_ty>(&self.#member)?,
            });
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
