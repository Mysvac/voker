use proc_macro2::TokenStream;
use quote::quote;
use syn::{Expr, Lit, MetaNameValue, spanned::Spanned};

use crate::path::fp::OptionFP;

/// A struct used to represent a type's documentation, if any.
///
/// This corresponds to `voker_reflect::info::TypePath::docs`.
///
/// By default, this will use the content of `#[doc = "..."]`, including the standard `/// ...` format.
/// But if the user explicitly adds `#[reflect(doc = "...")]`, this will switch to the custom document.
///
/// `enabled` field will only be true when feature `reflect_docs` is enabled.
#[derive(Debug)]
pub(crate) struct ReflectDocs {
    enabled: bool,
    is_custom: bool,
    docs: Vec<String>,
}

impl Default for ReflectDocs {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl ReflectDocs {
    #[inline]
    pub const fn new() -> Self {
        Self {
            #[cfg(feature = "reflect_docs")]
            enabled: true,
            #[cfg(not(feature = "reflect_docs"))]
            enabled: false,
            is_custom: false,
            docs: Vec::new(),
        }
    }

    /// Parse reflect attribute docs.
    ///
    /// This function do **not** check if the key is `docs`,
    /// it is guaranteed by the caller.
    ///
    /// Examples:
    /// - `#[doc = "..."]`
    pub fn parse_default_docs(&mut self, pair: &MetaNameValue) -> syn::Result<()> {
        if self.enabled && !self.is_custom {
            if let Expr::Lit(syn::ExprLit {
                lit: Lit::Str(lit_str),
                ..
            }) = &pair.value
            {
                self.docs.push(lit_str.value());
            } else {
                return Err(syn::Error::new(
                    pair.value.span(),
                    "`#[doc = ...]` expected a string literal value",
                ));
            }
        }
        Ok(())
    }

    /// Parse reflect attribute docs.
    ///
    /// This function do **not** check if the key is `docs`,
    /// it is guaranteed by the caller.
    ///
    /// Examples:
    /// - `#[reflect(doc = "...")]`
    /// - `#[reflect(doc = false)]`
    pub fn parse_custom_docs(&mut self, pair: &MetaNameValue) -> syn::Result<()> {
        if self.enabled {
            if let Expr::Lit(expr_lit) = &pair.value {
                match &expr_lit.lit {
                    Lit::Str(lit_str) => {
                        if !self.is_custom {
                            self.docs.clear();
                            self.is_custom = true;
                        }
                        self.docs.push(lit_str.value());
                    }
                    Lit::Bool(lit_bool) => {
                        if lit_bool.value() {
                            return Err(syn::Error::new(
                                expr_lit.span(),
                                "Explicit `true` is invalid, it's default value if `reflect_docs` feature is enabled.",
                            ));
                        }
                        self.enabled = lit_bool.value();
                        self.docs.clear();
                    }
                    _ => {
                        return Err(syn::Error::new(
                            expr_lit.span(),
                            "Expected a string or `false` literal",
                        ));
                    }
                }
            } else {
                return Err(syn::Error::new(
                    pair.value.span(),
                    "Expected a string or `false` literal",
                ));
            }
        }

        Ok(())
    }

    fn doc_string(&self) -> Option<String> {
        if !self.enabled || self.docs.is_empty() {
            return None;
        }

        let len = self.docs.len();
        let capacity = self.docs.iter().map(String::len).sum::<usize>() + len;
        if capacity == len {
            return None; // Empty document content
        }

        let mut res = String::with_capacity(capacity);
        for s in &self.docs {
            res.push_str(s);
            res.push('\n');
        }
        res.pop(); // delete the last `\n`

        Some(res)
    }

    /// If `reflect_docs` feature is disabled or `self.docs` is empty,
    /// this function will return an empty token stream.
    ///
    /// Otherwise, it will return content similar to this:
    ///
    /// ```ignore
    /// .with_docs(::core::option::Option::Some("......"))
    /// ```
    pub fn get_expression_with(&self) -> TokenStream {
        if let Some(doc) = self.doc_string() {
            quote! {
                .with_docs(#OptionFP::Some(#doc))
            }
        } else {
            crate::utils::empty()
        }
    }
}
