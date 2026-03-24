use proc_macro2::TokenStream;
use quote::quote;

use crate::derive_data::ReflectMeta;

/// Try `clone` or `reflect_clone` for `Reflect::apply`
pub(crate) fn get_common_apply_tokens(meta: &ReflectMeta, input: &syn::Ident) -> TokenStream {
    use crate::path::fp::{CloneFP, OptionFP, ResultFP};

    let voker_reflect_path = meta.voker_reflect_path();
    let reflect_ = crate::path::reflect_(voker_reflect_path);

    if meta.attrs().avail_traits.clone.is_some() {
        quote! {
            if let #OptionFP::Some(__val__) = <dyn #reflect_>::downcast_ref::<Self>(#input) {
                #CloneFP::clone_from(self, __val__);
                return #ResultFP::Ok(());
            }
        }
    } else {
        quote! {
            if <dyn #reflect_>::is::<Self>(#input)
                && let #ResultFP::Ok(__cloned__) = #reflect_::reflect_clone(#input)
                && let #ResultFP::Ok(__val__) = <dyn #reflect_>::take::<Self>(__cloned__)
            {
                *self = __val__;
                return #ResultFP::Ok(());
            }
        }
    }
}

/// Try `clone` or `reflect_clone` for `FromReflect::from_reflect`
pub(crate) fn get_common_from_reflect_tokens(
    meta: &ReflectMeta,
    input: &syn::Ident,
) -> TokenStream {
    use crate::path::fp::{CloneFP, OptionFP, ResultFP};

    let voker_reflect_path = meta.voker_reflect_path();
    let reflect_ = crate::path::reflect_(voker_reflect_path);

    if meta.attrs().avail_traits.clone.is_some() {
        quote! {
            if let #OptionFP::Some(__val__) = <dyn #reflect_>::downcast_ref::<Self>(#input) {
                return #OptionFP::Some(#CloneFP::clone(__val__));
            }
        }
    } else {
        quote! {
            if <dyn #reflect_>::is::<Self>(#input)
                && let #ResultFP::Ok(__cloned__) = #reflect_::reflect_clone(#input)
                && let #ResultFP::Ok(__val__) = <dyn #reflect_>::take::<Self>(__cloned__)
            {
                return #OptionFP::Some(__val__);
            }
        }
    }
}
