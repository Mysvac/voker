use proc_macro2::TokenStream;
use quote::quote;

#[inline]
pub(crate) fn type_path_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::info::TypePath
    }
}

#[inline(always)]
pub(crate) fn dynamic_type_path_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::info::DynamicTypePath
    }
}

#[inline(always)]
pub(crate) fn custom_attributes_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::info::CustomAttributes
    }
}

#[inline(always)]
pub(crate) fn const_param_info_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::info::ConstParamInfo
    }
}

#[inline(always)]
pub(crate) fn generic_info_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::info::GenericInfo
    }
}

#[inline(always)]
pub(crate) fn generics_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::info::Generics
    }
}

#[inline(always)]
pub(crate) fn type_param_info_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::info::TypeParamInfo
    }
}

#[inline(always)]
pub(crate) fn named_field_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::info::NamedField
    }
}

#[inline(always)]
pub(crate) fn unnamed_field_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::info::UnnamedField
    }
}

#[inline(always)]
pub(crate) fn opaque_info_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::info::OpaqueInfo
    }
}

#[inline(always)]
pub(crate) fn struct_info_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::info::StructInfo
    }
}

#[inline(always)]
pub(crate) fn tuple_struct_info_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::info::TupleStructInfo
    }
}

#[inline(always)]
pub(crate) fn struct_variant_info_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::info::StructVariantInfo
    }
}

#[inline(always)]
pub(crate) fn tuple_variant_info_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::info::TupleVariantInfo
    }
}

#[inline(always)]
pub(crate) fn unit_variant_info_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::info::UnitVariantInfo
    }
}

#[inline(always)]
pub(crate) fn variant_info_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::info::VariantInfo
    }
}

#[inline(always)]
pub(crate) fn variant_kind_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::info::VariantKind
    }
}

#[inline(always)]
pub(crate) fn enum_info_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::info::EnumInfo
    }
}

#[inline(always)]
pub(crate) fn reflect_kind_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::info::ReflectKind
    }
}

#[inline(always)]
pub(crate) fn type_info_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::info::TypeInfo
    }
}

#[inline(always)]
pub(crate) fn typed_(voker_reflect_path: &syn::Path) -> TokenStream {
    quote! {
        #voker_reflect_path::info::Typed
    }
}
