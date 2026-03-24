use crate::derive_data::ReflectMeta;

/// Generate `auto_register` implementation
#[cfg(feature = "auto_register")]
pub(crate) fn get_auto_register_impl(meta: &ReflectMeta) -> proc_macro2::TokenStream {
    use quote::quote;

    if let Some(span) = meta.attrs().auto_register {
        // Invalid for generic types.
        if meta.contains_generics() {
            return crate::utils::empty();
        }

        let voker_reflect_path = meta.voker_reflect_path();

        let auto_register_ = crate::path::auto_register_(voker_reflect_path, span);

        let real_ident = meta.real_ident();

        quote! {
            #auto_register_::inventory::submit!{
                #auto_register_::__AutoRegisterFunc(
                    <#real_ident as #auto_register_::__RegisterType>::__register
                )
            }
        }
    } else {
        crate::utils::empty()
    }
}

/// Generate `auto_register` implementation
#[cfg(not(feature = "auto_register"))]
pub(crate) fn get_auto_register_impl(_: &ReflectMeta) -> proc_macro2::TokenStream {
    crate::utils::empty()
}
