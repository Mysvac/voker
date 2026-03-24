use proc_macro2::Span;
use quote::{ToTokens, quote};
use syn::Ident;

use super::get_common_from_reflect_tokens;

use crate::derive_data::ReflectStruct;

// Generate `FromReflect::from_reflect` tokens for struct and tuple-struct.
pub(crate) fn impl_struct_from_reflect(
    info: &ReflectStruct,
    is_tuple: bool,
) -> proc_macro2::TokenStream {
    use crate::path::fp::{DefaultFP, OptionFP};
    let option_ = OptionFP.to_token_stream();

    let meta = info.meta();

    let voker_reflect_path = meta.voker_reflect_path();
    let reflect_ = crate::path::reflect_(voker_reflect_path);
    let from_reflect_ = crate::path::from_reflect_(voker_reflect_path);
    let reflect_ref_ = crate::path::reflect_ref_(voker_reflect_path);

    let struct_trait_path_ = if is_tuple {
        crate::path::tuple_struct_(voker_reflect_path)
    } else {
        crate::path::struct_(voker_reflect_path)
    };

    let struct_kind_ = if is_tuple {
        Ident::new("TupleStruct", Span::call_site())
    } else {
        Ident::new("Struct", Span::call_site())
    };

    let input_ = Ident::new("__input__", Span::call_site());

    let clone_tokens = get_common_from_reflect_tokens(meta, &input_);

    let (active_members, active_values): (Vec<_>, Vec<_>) = info
        .active_fields()
        .map(|field| {
            let member = field.to_member();
            let field_ty = field.data.ty.clone();
            let accessor = field.reflect_accessor();
            let value = quote! {
                match #struct_trait_path_::field(#input_, #accessor) {
                    #OptionFP::Some(__field) => <#field_ty as #from_reflect_>::from_reflect(__field),
                    #OptionFP::None => #OptionFP::None,
                }
            };
            (member, value)
        })
        .unzip();

    let constructor = if meta.attrs().avail_traits.default.is_some() {
        let check_field_len = if is_tuple {
            quote! {
                if <Self as #struct_trait_path_>::field_len(&__this) != #struct_trait_path_::field_len(#input_) {
                    return #OptionFP::None;
                }
            }
        } else {
            crate::utils::empty()
        };

        quote! {
            if let #reflect_ref_::#struct_kind_(#input_) = #reflect_::reflect_ref(#input_) {
                let mut __this = <Self as #DefaultFP>::default();
                #check_field_len
                #(
                    if let #option_::Some(__field_val) = #active_values {
                        __this.#active_members = __field_val;
                    }
                )*
                return #OptionFP::Some(__this);
            }
        }
    } else {
        quote! {
            if let #reflect_ref_::#struct_kind_(#input_) = #reflect_::reflect_ref(#input_) {
                let __this = Self {
                    #(#active_members: #active_values?,)*
                };
                return #OptionFP::Some(__this);
            }
        }
    };

    let real_ident = meta.real_ident();
    let (impl_generics, ty_generics, where_clause) = meta.split_generics(true, false, true);

    quote! {
        impl #impl_generics #from_reflect_ for #real_ident #ty_generics #where_clause  {
            fn from_reflect(#input_: &dyn #reflect_) -> #OptionFP<Self> {
                #clone_tokens

                #constructor

                #OptionFP::None
            }
        }
    }
}
