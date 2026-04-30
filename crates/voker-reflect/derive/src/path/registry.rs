use proc_macro2::TokenStream;
use quote::quote;

#[inline]
pub(crate) fn type_meta_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::registry::TypeMeta
    }
}

#[inline]
pub(crate) fn get_type_meta_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::registry::GetTypeMeta
    }
}

#[inline]
pub(crate) fn from_type_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::registry::FromType
    }
}

#[inline]
pub(crate) fn type_registry_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::registry::TypeRegistry
    }
}

#[inline]
pub(crate) fn reflect_default_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::registry::ReflectDefault
    }
}

#[inline]
pub(crate) fn reflect_from_reflect_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::registry::ReflectFromReflect
    }
}

#[inline]
pub(crate) fn reflect_serialize_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::registry::ReflectSerialize
    }
}

#[inline]
pub(crate) fn reflect_deserialize_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::registry::ReflectDeserialize
    }
}

#[inline]
pub(crate) fn reflect_convert_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::registry::ReflectConvert
    }
}
