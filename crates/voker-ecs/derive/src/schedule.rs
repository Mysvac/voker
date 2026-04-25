use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, parse_quote};

pub(crate) fn impl_derive_schedule_label(ast: DeriveInput) -> TokenStream {
    let voker_ecs_path = crate::path::voker_ecs();
    let trait_path = crate::path::schedule_label_(&voker_ecs_path);

    voker_macro_utils::derive_label(ast, trait_path)
}

pub(crate) fn impl_derive_system_set(ast: DeriveInput) -> TokenStream {
    use crate::path::fp::{CloneFP, DebugFP, EqFP, HashFP, SendFP, SyncFP};

    let voker_ecs_path = crate::path::voker_ecs();
    let system_set_ = crate::path::system_set_(&voker_ecs_path);
    let system_ = crate::path::system_(&voker_ecs_path);
    let system_set_begin_ = crate::path::system_set_begin_(&voker_ecs_path);
    let system_set_end_ = crate::path::system_set_end_(&voker_ecs_path);

    let type_ident = &ast.ident;

    match &ast.data {
        Data::Struct(_) | Data::Enum(_) => {}
        _ => {
            return syn::Error::new_spanned(
                type_ident,
                "`SystemSet` can only be derived for structs or enums",
            )
            .to_compile_error()
            .into();
        }
    }

    let mut generics = ast.generics;
    if generics.type_params().next().is_some() {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote! { Self: #SendFP + #SyncFP + #CloneFP + #DebugFP + #HashFP + #EqFP + 'static });
    } else if generics.lifetimes().next().is_some() {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote! { Self: 'static });
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        const _: () = {
            extern crate alloc as __alloc;

            impl #impl_generics #system_set_ for #type_ident #ty_generics #where_clause {
                fn begin(&self) -> __alloc::boxed::Box<dyn #system_<Input = (), Output = ()>> {
                    __alloc::boxed::Box::new(#system_set_begin_::<Self>::new(self.intern()))
                }

                fn end(&self) -> __alloc::boxed::Box<dyn #system_<Input = (), Output = ()>> {
                    __alloc::boxed::Box::new(#system_set_end_::<Self>::new(self.intern()))
                }

                fn dyn_clone(&self) -> __alloc::boxed::Box<dyn #system_set_> {
                    __alloc::boxed::Box::new(#CloneFP::clone(self))
                }
            }
        };
    }
    .into()
}
