use proc_macro2::TokenStream;
use quote::quote;

use crate::derive_data::ReflectMeta;

/// Generate implementation code for `Reflect` trait.
///
/// For param `add_from_reflect`, See [`ReflectMeta::split_generics`]
pub(crate) fn impl_trait_reflect(
    meta: &ReflectMeta,
    reflect_kind_token: TokenStream,
    apply_tokens: TokenStream,
    to_dynamic_tokens: TokenStream,
    reflect_clone_tokens: TokenStream,
    reflect_eq_tokens: TokenStream,
    reflect_cmp_tokens: TokenStream,
    reflect_hash_tokens: TokenStream,
    reflect_debug_tokens: TokenStream,
    add_from_reflect: bool,
) -> TokenStream {
    let voker_reflect_path = meta.voker_reflect_path();

    let reflect_ = crate::path::reflect_(voker_reflect_path);
    let macro_utils_ = crate::path::macro_utils_(voker_reflect_path);
    let reflect_kind_ = crate::path::reflect_kind_(voker_reflect_path);
    let reflect_ref_ = crate::path::reflect_ref_(voker_reflect_path);
    let reflect_mut_ = crate::path::reflect_mut_(voker_reflect_path);
    let reflect_owned_ = crate::path::reflect_owned_(voker_reflect_path);

    let real_ident = meta.real_ident();
    let (impl_generics, ty_generics, where_clause) =
        meta.split_generics(true, false, add_from_reflect);

    quote! {
        impl #impl_generics #reflect_ for #real_ident #ty_generics #where_clause {
            fn set(&mut self, value: #macro_utils_::Box<dyn #reflect_>) -> ::core::result::Result<(), #macro_utils_::Box<dyn #reflect_>> {
                *self = value.take::<Self>()?;
                Ok(())
            }

            #[inline]
            fn reflect_kind(&self) -> #reflect_kind_ {
                #reflect_kind_::#reflect_kind_token
            }

            #[inline]
            fn reflect_ref(&self) -> #reflect_ref_<'_> {
                #reflect_ref_::#reflect_kind_token(self)
            }

            #[inline]
            fn reflect_mut(&mut self) -> #reflect_mut_<'_> {
                #reflect_mut_::#reflect_kind_token(self)
            }

            #[inline]
            fn reflect_owned(self: #macro_utils_::Box<Self>) -> #reflect_owned_ {
                #reflect_owned_::#reflect_kind_token(self)
            }

            #to_dynamic_tokens

            #apply_tokens

            #reflect_clone_tokens

            #reflect_eq_tokens

            #reflect_cmp_tokens

            #reflect_hash_tokens

            #reflect_debug_tokens
        }
    }
}
