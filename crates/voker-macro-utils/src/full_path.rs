//! Full-path marker types for common core items.
//!
//! Each marker type in this module implements [`quote::ToTokens`].
//! This allows proc-macro code to emit stable, absolute paths such as
//! `::core::any::Any` without manually writing those segments repeatedly.
//!
//! # Examples
//!
//! ```no_run
//! use quote::quote;
//! use voker_macro_utils::full_path::AnyFP;
//!
//! let tokens = quote!(#AnyFP);
//! assert_eq!(tokens.to_string(), ":: core :: any :: Any");
//! ```

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

macro_rules! define_fp {
    ($($name:ident => $fp:path,)*) => {
        $(
            #[doc = concat!("Full Path for [`", stringify!($fp), "`]")]
            pub struct $name;

            impl ToTokens for $name {
                fn to_tokens(&self, tokens: &mut TokenStream) {
                    tokens.extend(quote!($fp));
                }
            }
        )*

    };
}

define_fp! {
    AnyFP => ::core::any::Any,
    CloneFP => ::core::clone::Clone,
    DefaultFP => ::core::default::Default,
    OptionFP => ::core::option::Option,
    ResultFP => ::core::result::Result,
    SendFP => ::core::marker::Send,
    SyncFP => ::core::marker::Sync,
    CopyFP => ::core::marker::Copy,
    PartialEqFP => ::core::cmp::PartialEq,
    PartialOrdFP => ::core::cmp::PartialOrd,
    EqFP => ::core::cmp::Eq,
    OrdFP => ::core::cmp::Ord,
    HashFP => ::core::hash::Hash,
    HasherFP => ::core::hash::Hasher,
    DebugFP => ::core::fmt::Debug,
    DisplayFP => ::core::fmt::Display,
    ErrorFP => ::core::error::Error,
    TypeIdFP => ::core::any::TypeId,
    FromFP => ::core::convert::From,
    IntoFP => ::core::convert::Into,
}
