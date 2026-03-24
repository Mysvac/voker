use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{LitStr, spanned::Spanned};

/// An enum representing different types of string expressions
#[derive(Clone)]
pub(crate) enum StringExpr {
    /// A string that is valid at compile time.
    ///
    /// In most cases, this is a string lit, such as: `"mystring"`.
    ///
    /// But sometimes, this also includes macros, such as: `module_path!(xxx)`
    Const(TokenStream),
    /// A [string slice](str) that is borrowed for a `'static` lifetime.
    ///
    /// For example: `a`, a is a `&'static str`.
    Borrowed(TokenStream),
    /// An [owned string](String).
    ///
    /// For example: `a`, a is a [`String`].
    Owned(TokenStream),
}

impl Default for StringExpr {
    fn default() -> Self {
        Self::Const("".to_token_stream())
    }
}

impl<T: ToString + Spanned> From<T> for StringExpr {
    fn from(value: T) -> Self {
        Self::Const(LitStr::new(&value.to_string(), value.span()).to_token_stream())
    }
}

impl StringExpr {
    /// Creates a [constant] [`StringExpr`] from a [`struct@LitStr`].
    ///
    /// [constant]: StringExpr::Const
    pub fn from_lit(lit: &LitStr) -> Self {
        Self::Const(lit.to_token_stream())
    }

    /// Creates a [constant] [`StringExpr`] by interpreting a [string slice][str] as a [`struct@LitStr`].
    ///
    /// [constant]: StringExpr::Const
    pub fn from_str(string: &str) -> Self {
        // ↓ Generate tokens with string literal.
        Self::Const(string.to_token_stream())
    }

    /// Returns tokens for a statically borrowed [string slice](str).
    pub fn into_borrowed(self) -> TokenStream {
        match self {
            Self::Const(tokens) | Self::Borrowed(tokens) => tokens,
            Self::Owned(owned) => quote! {
                &#owned as &str
            },
        }
    }

    /// Returns tokens for an [owned string](String).
    pub fn into_owned(self, voker_reflect_path: &syn::Path) -> TokenStream {
        match self {
            Self::Const(tokens) | Self::Borrowed(tokens) => {
                let macro_utils_ = crate::path::macro_utils_(voker_reflect_path);
                quote! {
                    #macro_utils_::ToOwned::to_owned(#tokens)
                }
            }
            Self::Owned(owned) => owned,
        }
    }

    /// Get inner TokenStream if self is const string expr.
    ///
    /// # Panic
    /// - self is not const string expr.
    fn into_const(self) -> TokenStream {
        match self {
            StringExpr::Const(token_stream) => token_stream,
            _ => unreachable!(), // See: [`StringExpr::from_iter`]
        }
    }

    /// Check if self is const expression
    fn is_const(&self) -> bool {
        matches!(self, StringExpr::Const(_))
    }

    /// concat string from iterator
    ///
    /// If all expressions are [`StringExpr::Const`] this will use [`concat`] to merge them.
    pub fn from_iter<T: IntoIterator<Item = StringExpr>>(
        iter: T,
        voker_reflect_path: &syn::Path,
    ) -> Self {
        let exprs: Vec<StringExpr> = iter.into_iter().collect();

        if exprs.is_empty() {
            return Self::default();
        }

        if exprs.iter().all(|expr| expr.is_const()) {
            let inner = exprs.into_iter().map(StringExpr::into_const); // `exprs` will not be empty here.

            Self::Const(quote! {
                ::core::concat!( #(#inner),* )
            })
        } else {
            let macro_utils_ = crate::path::macro_utils_(voker_reflect_path);
            let inner = exprs.into_iter().map(StringExpr::into_borrowed);

            Self::Owned(quote! {
                #macro_utils_::__concat(&[ #(#inner),* ])
            })
        }
    }
}
