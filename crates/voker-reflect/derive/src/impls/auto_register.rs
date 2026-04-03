use crate::derive_data::ReflectMeta;
use proc_macro2::TokenStream;

/// Generate `auto_register` implementation
pub(crate) fn get_auto_register_impl(meta: &ReflectMeta) -> TokenStream {
    use quote::quote;

    if !meta.attrs().impl_switchs.impl_get_type_meta {
        return crate::utils::empty();
    }

    // Invalid for generic types.
    if meta.contains_generics() {
        return crate::utils::empty();
    }

    let real_ident = meta.real_ident();
    let voker_reflect_path = meta.voker_reflect_path();
    let macro_utils_ = crate::path::macro_utils_(voker_reflect_path);

    quote! {
        impl #macro_utils_::AutoRegister for #real_ident {}

        #macro_utils_::inv::submit!{
            #macro_utils_::RegisterFn::of::<#real_ident>()
            => #macro_utils_::RegisterFn
        }
    }
}
