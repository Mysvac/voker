use proc_macro2::Span;

use syn::{Attribute, MacroDelimiter, Meta, MetaList};
use syn::{MetaNameValue, Token, parse::ParseStream};

use super::{CustomAttributes, ReflectDocs};

use crate::REFLECT_ATTRIBUTE;

mod kw {
    syn::custom_keyword!(doc);
    syn::custom_keyword!(skip_serde);
}

#[derive(Default)]
pub(crate) struct FieldAttributes {
    /// Custom attributes: `#[reflect(@...)]`.
    pub custom_attributes: CustomAttributes,
    /// Custom docs: `#[doc = ""]` or `#[reflect(docs = "")]`
    pub docs: ReflectDocs,
    /// Determines how this field should be skipped during reflect (de)serialization.
    pub skip_serde: Option<Span>,
}

impl FieldAttributes {
    pub fn parse_attrs(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut field_attributes = FieldAttributes::default();

        for attribute in attrs {
            match &attribute.meta {
                Meta::List(meta_list) if meta_list.path.is_ident(REFLECT_ATTRIBUTE) => {
                    if !matches!(&meta_list.delimiter, MacroDelimiter::Paren(_)) {
                        return Err(syn::Error::new(
                            meta_list.delimiter.span().join(),
                            format_args!(
                                "`#[{REFLECT_ATTRIBUTE}(\"...\")]` must use parentheses `(` and `)`"
                            ),
                        ));
                    }

                    field_attributes.parse_meta_list(meta_list)?;
                }
                Meta::NameValue(pair)
                    if ::core::cfg!(feature = "reflect_docs") && pair.path.is_ident("doc") =>
                {
                    field_attributes.docs.parse_default_docs(pair)?;
                }
                _ => continue,
            }
        }

        Ok(field_attributes)
    }

    fn parse_meta_list(&mut self, meta: &MetaList) -> syn::Result<()> {
        // meta.parse_nested_meta(||)
        meta.parse_args_with(|stream: ParseStream| {
            loop {
                if stream.is_empty() {
                    break;
                }
                self.parse_inner_attribute(stream)?;
                if stream.is_empty() {
                    break;
                }
                stream.parse::<Token![,]>()?;
            }
            Ok(())
        })
    }

    fn parse_inner_attribute(&mut self, input: ParseStream) -> syn::Result<()> {
        let lookahead = input.lookahead1();
        if lookahead.peek(Token![@]) {
            self.parse_custom_attribute(input)
        } else if lookahead.peek(kw::doc) {
            self.parse_docs(input)
        } else if lookahead.peek(kw::skip_serde) {
            self.parse_skip_serde(input)
        } else {
            Err(lookahead.error())
        }
    }

    fn parse_custom_attribute(&mut self, input: ParseStream) -> syn::Result<()> {
        self.custom_attributes.parse_stream(input)
    }

    /// This function can be used when the `reflect_docs` feature is disabled.
    /// When the feature is not enabled, it will not do anything.
    fn parse_docs(&mut self, input: ParseStream) -> syn::Result<()> {
        // #[reflect(docs = "...")]
        let pair = input.parse::<MetaNameValue>()?;
        self.docs.parse_custom_docs(&pair)
    }

    fn parse_skip_serde(&mut self, input: ParseStream) -> syn::Result<()> {
        let s = input.parse::<kw::skip_serde>()?.span;
        self.skip_serde = Some(s);
        Ok(())
    }
}
