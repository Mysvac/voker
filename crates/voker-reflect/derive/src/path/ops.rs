use proc_macro2::TokenStream;
use quote::quote;

#[inline]
pub(crate) fn apply_error_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::ops::ApplyError
    }
}

#[inline]
pub(crate) fn reflect_clone_error_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::ops::ReflectCloneError
    }
}

#[inline]
pub(crate) fn reflect_mut_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::ops::ReflectMut
    }
}

#[inline]
pub(crate) fn reflect_owned_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::ops::ReflectOwned
    }
}

#[inline]
pub(crate) fn reflect_ref_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::ops::ReflectRef
    }
}

#[inline]
pub(crate) fn dynamic_struct_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::ops::DynamicStruct
    }
}

#[inline]
pub(crate) fn struct_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::ops::Struct
    }
}

#[inline]
pub(crate) fn struct_field_iter_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::ops::StructFieldIter
    }
}

#[inline]
pub(crate) fn struct_apply_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::impls::struct_apply
    }
}

#[inline]
pub(crate) fn struct_hash_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::impls::struct_hash
    }
}

#[inline]
pub(crate) fn struct_debug_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::impls::struct_debug
    }
}

#[inline]
pub(crate) fn struct_eq_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::impls::struct_eq
    }
}

#[inline]
pub(crate) fn struct_cmp_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::impls::struct_cmp
    }
}

#[inline]
pub(crate) fn dynamic_tuple_struct_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::ops::DynamicTupleStruct
    }
}

#[inline]
pub(crate) fn tuple_struct_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::ops::TupleStruct
    }
}

#[inline]
pub(crate) fn tuple_struct_field_iter_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::ops::TupleStructFieldIter
    }
}

#[inline]
pub(crate) fn tuple_struct_apply_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::impls::tuple_struct_apply
    }
}

#[inline]
pub(crate) fn tuple_struct_hash_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::impls::tuple_struct_hash
    }
}

#[inline]
pub(crate) fn tuple_struct_debug_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::impls::tuple_struct_debug
    }
}

#[inline]
pub(crate) fn tuple_struct_eq_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::impls::tuple_struct_eq
    }
}

#[inline]
pub(crate) fn tuple_struct_cmp_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::impls::tuple_struct_cmp
    }
}

#[inline]
pub(crate) fn variant_field_iter_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::ops::VariantFieldIter
    }
}

#[inline]
pub(crate) fn enum_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::ops::Enum
    }
}

#[inline]
pub(crate) fn enum_apply_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::impls::enum_apply
    }
}

#[inline]
pub(crate) fn enum_debug_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::impls::enum_debug
    }
}

#[inline]
pub(crate) fn enum_eq_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::impls::enum_eq
    }
}

#[inline]
pub(crate) fn enum_cmp_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::impls::enum_cmp
    }
}

#[inline]
pub(crate) fn enum_hash_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::impls::enum_hash
    }
}
